pub mod color;
pub mod fluid;
pub mod lines;
pub mod noise;
pub mod texture;
pub mod view;

pub use view::ScreenViewport;
pub use view::ViewTransform;

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
