use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::WebGl2RenderingContext as GL;
use web_sys::Window;

pub fn get_rendering_context(element_id: &str) -> Result<(Canvas, GL, u32, u32, f64), JsValue> {
    set_panic_hook();

    let window = window();
    let document = window.document().unwrap();
    let html_canvas = document.get_element_by_id(element_id).unwrap();
    let html_canvas: web_sys::HtmlCanvasElement =
        html_canvas.dyn_into::<web_sys::HtmlCanvasElement>()?;

    let pixel_ratio: f64 = window.device_pixel_ratio().min(1.5);
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

    let gl = canvas
        .get_context_with_context_options("webgl2", &options)?
        .unwrap()
        .dyn_into::<GL>()?;
    gl.get_extension("OES_texture_float")?;
    gl.get_extension("OES_texture_float_linear")?;
    gl.get_extension("EXT_color_buffer_float")?;
    gl.get_extension("EXT_float_blend")?;

    gl.disable(GL::BLEND);
    gl.disable(GL::DEPTH_TEST);

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

    pub fn client_width(&self) -> i32 {
        match self {
            Canvas::OnscreenCanvas(ref canvas) => canvas.client_width(),
            Canvas::OffscreenCanvas(ref html_canvas, _) => html_canvas.client_width(),
        }
    }

    pub fn client_height(&self) -> i32 {
        match self {
            Canvas::OnscreenCanvas(ref canvas) => canvas.client_height(),
            Canvas::OffscreenCanvas(ref html_canvas, _) => html_canvas.client_height(),
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
