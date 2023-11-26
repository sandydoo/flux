pub mod fluid;
pub mod lines;

pub struct GraphicsContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub downlevel_caps: wgpu::DownlevelCapabilities,
    pub color_format: wgpu::TextureFormat,
    pub screen_size: wgpu::Extent3d,
}

pub struct Render {
    pub lines: lines::Context,
    pub fluid: fluid::Context,
}

impl Render {
    pub fn new(ctx: &GraphicsContext) -> Self {
        let lines = lines::Context::new(ctx);
        let fluid = fluid::Context::new(ctx);

        Self { lines, fluid }
    }
}
