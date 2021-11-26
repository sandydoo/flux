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
