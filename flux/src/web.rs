use serde::Serialize;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::Window;

#[derive(Serialize)]
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
    OffscreenCanvas(web_sys::OffscreenCanvas),
}

impl Canvas {
    pub fn new(html_canvas: web_sys::HtmlCanvasElement) -> Self {
        match html_canvas.transfer_control_to_offscreen() {
            Ok(offscreen_canvas) => Canvas::OffscreenCanvas(offscreen_canvas),
            Err(_) => Canvas::OnscreenCanvas(html_canvas),
        }
    }

    pub fn get_context_with_context_options(
        &self,
        context_id: &str,
        context_options: &JsValue,
    ) -> Result<Option<js_sys::Object>, JsValue> {
        match self {
            Canvas::OnscreenCanvas(canvas) => {
                canvas.get_context_with_context_options(context_id, context_options)
            }
            Canvas::OffscreenCanvas(canvas) => {
                canvas.get_context_with_context_options(context_id, context_options)
            }
        }
    }
}

pub fn window() -> Window {
    web_sys::window().expect("The global `window` doesn’t exist")
}

pub fn request_animation_frame(f: &Closure<dyn FnMut(f32)>) {
    window()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("should register `requestAnimationFrame` OK");
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
