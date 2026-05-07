use std::borrow::Cow;

use crate::BackendCaps;

pub mod color;
pub mod fluid;
pub mod lines;
pub mod noise;
pub mod texture;
pub mod view;

pub use view::ScreenViewport;
pub use view::ViewTransform;

/// Returns WGSL source with `r32float` / `rg32float` write-only storage texture
/// formats rewritten to `rgba16float` when `FLOAT32_FILTERABLE` isn't available.
///
/// `rgba16float` is the only 16-bit float format that is both filterable and
/// usable as a storage texture across the baseline wgpu format tier — `r16float`
/// and `rg16float` aren't guaranteed to permit `STORAGE_BINDING`, so they aren't
/// safe fallbacks even on hardware that supports linear filtering of them.
/// We trade some texel bandwidth (4× for pressure, 2× for noise) for
/// compatibility. The shaders only read `.x` / `.xy` and zero-fill the unused
/// channels at write time, so widening the format is a no-op semantically.
///
/// When the feature is available, the source passes through unchanged.
///
/// Panics if the source contains neither token: a renamed shader or removed
/// storage declaration should fail loudly rather than silently apply nothing.
pub(crate) fn downgrade_float_storage(
    source: &'static str,
    caps: BackendCaps,
) -> Cow<'static, str> {
    if caps.float32_filterable {
        return Cow::Borrowed(source);
    }
    let patched = source
        .replace(
            "texture_storage_2d<r32float, write>",
            "texture_storage_2d<rgba16float, write>",
        )
        .replace(
            "texture_storage_2d<rg32float, write>",
            "texture_storage_2d<rgba16float, write>",
        );
    assert_ne!(
        patched, source,
        "expected r32float/rg32float storage format substitution, but neither \
         token was present in the WGSL source"
    );
    Cow::Owned(patched)
}

// pub struct GraphicsContext {
//     pub device: wgpu::Device,
//     pub queue: wgpu::Queue,
//     pub color_format: wgpu::TextureFormat,
//     pub screen_size: wgpu::Extent3d,
// }

// pub struct Render {
//     pub lines: lines::Context,
//     pub fluid: fluid::Context,
// }
//
// impl Render {
//     pub fn new(ctx: &GraphicsContext) -> Self {
//         let lines = lines::Context::new(ctx);
//         let fluid = fluid::Context::new(ctx);
//
//         Self { lines, fluid }
//     }
// }
