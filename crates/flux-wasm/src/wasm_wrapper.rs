use serde::Serialize;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;
use web_sys::Window;

#[wasm_bindgen]
pub struct Flux {
    canvas: Canvas,
    context: Rc<glow::Context>,
    width: u32,
    height: u32,
    pixel_ratio: f64,
    id: flux::Flux,
}

#[wasm_bindgen]
impl Flux {
    #[wasm_bindgen(setter)]
    pub fn set_settings(&mut self, settings_object: &JsValue) {
        let settings: flux::settings::Settings = settings_object.into_serde().unwrap();
        self.id.update(&Rc::new(settings));
    }

    #[wasm_bindgen(constructor)]
    pub fn new(settings_object: &JsValue) -> Result<Flux, JsValue> {
        let (canvas, gl, width, height, pixel_ratio) = get_rendering_context("canvas")?;
        let context = Rc::new(gl);

        let settings: Rc<flux::settings::Settings> = match settings_object.into_serde() {
            Ok(settings) => Rc::new(settings),
            Err(msg) => return Err(JsValue::from_str(&msg.to_string())),
        };

        let flux = flux::Flux::new(&context, width, height, &settings)
            .map_err(|_err| JsValue::from_str("failed"))?;

        Ok(Self {
            id: flux,
            canvas,
            width,
            height,
            pixel_ratio,
            context,
        })
    }

    pub fn animate(&mut self, timestamp: f32) {
        self.id.animate(timestamp);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let new_width = (self.pixel_ratio * f64::from(width)) as u32;
        let new_height = (self.pixel_ratio * f64::from(height)) as u32;

        if (self.width != new_width) || (self.height != new_height) {
            self.canvas.set_width(new_width);
            self.canvas.set_height(new_height);
            self.id.resize(new_width, new_height);
            self.width = new_width;
            self.height = new_height;
        }
    }
}

pub fn get_rendering_context(
    element_id: &str,
) -> Result<(Canvas, glow::Context, u32, u32, f64), JsValue> {
    use wasm_bindgen::JsCast;
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

    let pixel_ratio: f64 = window.device_pixel_ratio(); //.min(1.5);
    let client_width = html_canvas.client_width();
    let client_height = html_canvas.client_height();
    let width = (pixel_ratio * f64::from(client_width)) as u32;
    let height = (pixel_ratio * f64::from(client_height)) as u32;
    html_canvas.set_width(width);
    html_canvas.set_height(height);

    let canvas = Canvas::new(html_canvas.clone());

    let options = ContextOptions {
        // Disabling alpha can lead to poor performance on some platforms.
        // We also need it for MSAA.
        alpha: true,
        depth: false,
        stencil: false,
        desynchronized: false,
        antialias: false,
        fail_if_major_performance_caveat: false,
        power_preference: "high-performance",
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
            &"Can’t create the WebGl2 rendering context",
        ));
    };

    Ok((canvas, gl, width, height, pixel_ratio))
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
        match html_canvas.transfer_control_to_offscreen() {
            Ok(offscreen_canvas) => Canvas::OffscreenCanvas(html_canvas, offscreen_canvas),
            Err(_) => Canvas::OnscreenCanvas(html_canvas),
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

#[macro_export]
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}
