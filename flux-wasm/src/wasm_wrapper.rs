use flux::{self, settings};
use gloo_utils::format::JsValueSerdeExt;
use glow::HasContext;
use serde::Serialize;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::Window;

#[wasm_bindgen]
pub struct Flux {
    canvas: Canvas,
    #[allow(dead_code)]
    context: Rc<glow::Context>,
    logical_width: u32,
    logical_height: u32,
    pixel_ratio: f64,
    instance: flux::Flux,
}

#[wasm_bindgen]
impl Flux {
    #[wasm_bindgen(setter)]
    pub fn set_settings(&mut self, settings_object: &JsValue) {
        let settings: settings::Settings = settings_object.into_serde().unwrap();
        self.instance.update(&Rc::new(settings));
    }

    #[wasm_bindgen]
    pub fn save_image(&mut self, bitmap: web_sys::ImageBitmap) {
        self.instance.sample_colors_from_image_bitmap(&bitmap);
    }

    #[wasm_bindgen(constructor)]
    pub fn new(settings_object: &JsValue) -> Result<Flux, JsValue> {
        console_log::init_with_level(log::Level::Debug).expect("cannot enable logging");

        let (
            canvas,
            gl,
            logical_width,
            logical_height,
            physical_width,
            physical_height,
            pixel_ratio,
        ) = get_rendering_context("canvas")?;
        let context = Rc::new(gl);
        let settings: Rc<settings::Settings> = match settings_object.into_serde() {
            Ok(settings) => Rc::new(settings),
            Err(msg) => return Err(JsValue::from_str(&msg.to_string())),
        };

        let flux = flux::Flux::new(
            &context,
            logical_width,
            logical_height,
            physical_width,
            physical_height,
            &settings,
        )
        .map_err(|err| JsValue::from_str(&err.to_string()))?;

        Ok(Self {
            instance: flux,
            canvas,
            logical_width,
            logical_height,
            pixel_ratio,
            context,
        })
    }

    pub fn animate(&mut self, timestamp: f64) {
        self.instance.animate(timestamp);
    }

    pub fn resize(&mut self, logical_width: u32, logical_height: u32) {
        if (self.logical_width != logical_width) || (self.logical_height != logical_height) {
            let (physical_width, physical_height) =
                physical_from_logical_size(logical_width, logical_height, self.pixel_ratio);

            self.canvas.set_width(physical_width);
            self.canvas.set_height(physical_height);

            self.instance.resize(
                logical_width,
                logical_height,
                physical_width,
                physical_height,
            );

            self.logical_width = logical_width;
            self.logical_height = logical_height;
        }
    }
}

pub fn get_rendering_context(
    element_id: &str,
) -> Result<(Canvas, glow::Context, u32, u32, u32, u32, f64), JsValue> {
    use web_sys::WebGl2RenderingContext as GL;

    set_panic_hook();

    let window = window();
    let document = window.document().expect("I expected to find a document");
    let html_canvas = document.get_element_by_id(element_id).unwrap_or_else(|| {
        panic!(
            "I expected to find a canvas element with id `{}`",
            element_id
        )
    });
    let html_canvas: web_sys::HtmlCanvasElement =
        html_canvas.dyn_into::<web_sys::HtmlCanvasElement>()?;

    let pixel_ratio: f64 = window.device_pixel_ratio();
    let logical_width = html_canvas.client_width() as u32;
    let logical_height = html_canvas.client_height() as u32;
    let (physical_width, physical_height) =
        physical_from_logical_size(logical_width, logical_height, pixel_ratio);
    html_canvas.set_width(physical_width);
    html_canvas.set_height(physical_height);

    let canvas = Canvas::new(html_canvas);

    let options = ContextOptions {
        // Disabling alpha can lead to poor performance on some platforms.
        alpha: true,
        depth: false,
        stencil: false,
        desynchronized: false,
        antialias: false,
        fail_if_major_performance_caveat: false,
        // TODO: Revert to high-performance once dual-GPU issues on Chrome are
        // resolved.
        power_preference: "default",
        premultiplied_alpha: true,
        preserve_drawing_buffer: false,
    }
    .serialize();

    let gl = if let Ok(Some(gl)) = canvas.get_context_with_context_options("webgl2", &options) {
        let gl = gl.dyn_into::<GL>()?;
        gl.get_extension("OES_texture_float")?;
        gl.get_extension("OES_texture_float_linear")?;
        gl.get_extension("EXT_color_buffer_float")?;
        gl.get_extension("EXT_float_blend")?;

        gl.disable(GL::BLEND);
        gl.disable(GL::DEPTH_TEST);

        glow::Context::from_webgl2_context(gl)
    } else {
        return Err(JsValue::from_str(
            "Can’t create the WebGl2 rendering context",
        ));
    };

    Ok((
        canvas,
        gl,
        logical_width,
        logical_height,
        physical_width,
        physical_height,
        pixel_ratio,
    ))
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContextOptions {
    pub alpha: bool,
    pub depth: bool,
    pub stencil: bool,
    pub desynchronized: bool,
    pub antialias: bool,
    pub fail_if_major_performance_caveat: bool,
    pub power_preference: &'static str,
    pub premultiplied_alpha: bool,
    pub preserve_drawing_buffer: bool,
}

impl ContextOptions {
    pub fn serialize(&self) -> JsValue {
        // TODO: deal with result
        JsValue::from_serde(self).unwrap()
    }
}

// An offscreen canvas decouples our canvas from the DOM. Not having to sync
// with the DOM greatly improves performance, but browser support is poor.
// AFAIK, Chrome is the only browser that has implemented the feature.
pub enum Canvas {
    OnscreenCanvas(web_sys::HtmlCanvasElement),
    OffscreenCanvas(web_sys::HtmlCanvasElement, web_sys::OffscreenCanvas),
}

impl Canvas {
    pub fn new(html_canvas: web_sys::HtmlCanvasElement) -> Self {
        // WARN: Panics in Safari.
        // match html_canvas.transfer_control_to_offscreen() {
        //     Ok(offscreen_canvas) => Canvas::OffscreenCanvas(html_canvas, offscreen_canvas),
        //     Err(_) => Canvas::OnscreenCanvas(html_canvas),
        // }

        Canvas::OnscreenCanvas(html_canvas)
    }

    pub fn get_context(&self, context_id: &str) -> Result<Option<js_sys::Object>, JsValue> {
        match self {
            Canvas::OnscreenCanvas(ref canvas) => canvas.get_context(context_id),
            Canvas::OffscreenCanvas(_, ref canvas) => canvas.get_context(context_id),
        }
    }

    pub fn get_context_with_context_options(
        &self,
        context_id: &str,
        context_options: &JsValue,
    ) -> Result<Option<js_sys::Object>, JsValue> {
        match self {
            Canvas::OnscreenCanvas(ref canvas) => {
                canvas.get_context_with_context_options(context_id, context_options)
            }
            Canvas::OffscreenCanvas(_, ref canvas) => {
                canvas.get_context_with_context_options(context_id, context_options)
            }
        }
    }

    pub fn set_width(&self, width: u32) {
        match self {
            Canvas::OnscreenCanvas(ref canvas) => canvas.set_width(width),
            Canvas::OffscreenCanvas(_, ref canvas) => canvas.set_width(width),
        }
    }

    pub fn set_height(&self, height: u32) {
        match self {
            Canvas::OnscreenCanvas(ref canvas) => canvas.set_height(height),
            Canvas::OffscreenCanvas(_, ref canvas) => canvas.set_height(height),
        }
    }
}

pub fn window() -> Window {
    web_sys::window().expect("The global `window` doesn’t exist")
}

// https://github.com/rustwasm/console_error_panic_hook#readme
pub fn set_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

fn physical_from_logical_size(
    logical_width: u32,
    logical_height: u32,
    pixel_ratio: f64,
) -> (u32, u32) {
    (
        (pixel_ratio * f64::from(logical_width)) as u32,
        (pixel_ratio * f64::from(logical_height)) as u32,
    )
}
