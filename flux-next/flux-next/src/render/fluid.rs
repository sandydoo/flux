use crate::render::GraphicsContext;

pub struct Context {
    fluid_size: [u32; 2],

    velocity_textures: Vec<wgpu::Texture>,
    advection_forward_texture: wgpu::Texture,
    advection_reverse_texture: wgpu::Texture,
    divergence_texture: wgpu::Texture,
    pressure_textures: Vec<wgpu::Texture>,

    clear_pressure_to_pipeline: wgpu::ComputePipeline,
    advection_pipeline: wgpu::ComputePipeline,
    adjust_advection_pipeline: wgpu::ComputePipeline,
    diffusion_pipeline: wgpu::ComputePipeline,
    divergence_pipeline: wgpu::ComputePipeline,
    pressure_pipeline: wgpu::ComputePipeline,
    subtract_gradient_pipeline: wgpu::ComputePipeline,
}

impl Context {
    pub fn new(ctx: &GraphicsContext, fluid_size: [u32; 2]) -> Self {
        let size = wgpu::Extent3d {
            width: fluid_size[0],
            height: fluid_size[1],
            depth_or_array_layers: 1,
        };

        let velocity_textures = vec![
            ctx.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("velocity"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rg16Float,
                view_formats: &[],
                usage: wgpu::TextureUsages::STORAGE | wgpu::TextureUsages::SAMPLED,
            });
            2
        ];

        let advection_forward_texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("advection forward"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rg16Float,
            view_formats: &[],
            usage: wgpu::TextureUsages::STORAGE | wgpu::TextureUsages::SAMPLED,
        });

        let advection_reverse_texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("advection reverse"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rg16Float,
            view_formats: &[],
            usage: wgpu::TextureUsages::STORAGE | wgpu::TextureUsages::SAMPLED,
        });

        let divergence_texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("divergence"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R16Float,
            view_formats: &[],
            usage: wgpu::TextureUsages::STORAGE | wgpu::TextureUsages::SAMPLED,
        });

        let pressure_textures = vec![
            ctx.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("pressure"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R16Float,
                view_formats: &[],
                usage: wgpu::TextureUsages::STORAGE | wgpu::TextureUsages::SAMPLED,
            });
            2
        ];

        Self {
            velocity_textures,
            advection_forward_texture,
            advection_reverse_texture,
            divergence_texture,
            pressure_textures,
        }
    }

    pub fn advect_forward(&self, timestep: f32) {}

    pub fn advect_reverse(&self, timestep: f32) {}

    pub fn adjust_advection(&self, timestep: f32) {}

    pub fn diffuse(&self, timestep: f32) {}

    pub fn calculate_divergence(&self) {}

    pub fn clear_pressure(&self) {}

    pub fn solve_pressure(&self) {}

    pub fn subtract_gradient(&self) {}
}
