#[cfg(target_arch = "wasm32")]
#[path = "wasm_wrapper.rs"]
mod flux_wasm;

#[cfg(target_arch = "wasm32")]
pub use flux_wasm::*;
