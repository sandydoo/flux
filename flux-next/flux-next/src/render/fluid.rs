use crate::settings::{self, Settings};

use std::borrow::Cow;
use std::rc::Rc;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct FluidUniforms {
    timestep: f32,
    dissipation: f32,
    alpha: f32,
    r_beta: f32,
    center_factor: f32,
    stencil_factor: f32,
    texel_size: [f32; 2],
}

pub struct Context {
    fluid_size: [f32; 2],
    fluid_size_3d: wgpu::Extent3d,

    pressure_mode: settings::PressureMode,
    pressure_iterations: u32,

    fluid_uniforms: FluidUniforms,
    fluid_uniform_buffer: wgpu::Buffer,

    velocity_textures: [wgpu::Texture; 2],
    velocity_texture_views: [wgpu::TextureView; 2],
    advection_forward_texture: wgpu::Texture,
    advection_forward_texture_view: wgpu::TextureView,
    advection_reverse_texture: wgpu::Texture,
    advection_reverse_texture_view: wgpu::TextureView,
    divergence_texture: wgpu::Texture,
    pressure_textures: [wgpu::Texture; 2],

    velocity_bind_groups: [wgpu::BindGroup; 2],
    uniform_bind_group: wgpu::BindGroup,
    advection_forward_bind_group: wgpu::BindGroup,
    advection_reverse_bind_group: wgpu::BindGroup,
    adjust_advection_bind_group: wgpu::BindGroup,
    divergence_bind_group: wgpu::BindGroup,
    divergence_sample_bind_group: wgpu::BindGroup,
    pressure_bind_groups: [wgpu::BindGroup; 2],

    clear_pressure_pipeline: wgpu::ComputePipeline,
    advection_pipeline: wgpu::ComputePipeline,
    adjust_advection_pipeline: wgpu::ComputePipeline,
    diffusion_pipeline: wgpu::ComputePipeline,
    divergence_pipeline: wgpu::ComputePipeline,
    pressure_pipeline: wgpu::ComputePipeline,
    subtract_gradient_pipeline: wgpu::ComputePipeline,
}

impl Context {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, settings: &Rc<Settings>) -> Self {
        let (width, height) = (
            // scaling_ratio.rounded_x() * settings.fluid_size,
            // scaling_ratio.rounded_y() * settings.fluid_size,
            settings.fluid_size,
            settings.fluid_size,
        );
        let texel_size = [1.0 / width as f32, 1.0 / height as f32];
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        // Uniforms

        // dx^2 / (rho * dt)
        let center_factor = 1.0 / (settings.viscosity * settings.fluid_timestep);
        let stencil_factor = 1.0 / (4.0 + center_factor);

        let fluid_uniforms = FluidUniforms {
            // timestep: 1.0 / settings.fluid_timestep,
            timestep: settings.fluid_timestep,
            dissipation: settings.velocity_dissipation,
            alpha: -1.0,
            r_beta: 0.25,
            center_factor,
            stencil_factor,
            texel_size,
        };
        let fluid_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniform:FluidUniforms"),
            contents: bytemuck::cast_slice(&[fluid_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Textures

        let velocity_textures = [
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("texture:velocity_0"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rg32Float,
                view_formats: &[],
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::COPY_DST,
            }),
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("texture:velocity_1"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rg32Float,
                view_formats: &[],
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::COPY_DST,
            }),
        ];

        let advection_forward_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("texture:advection_forward"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rg32Float,
            view_formats: &[],
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_DST,
        });

        let advection_reverse_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("texture:advection_reverse"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rg32Float,
            view_formats: &[],
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_DST,
        });

        let divergence_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("texture:divergence"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            view_formats: &[],
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_DST,
        });

        let pressure_textures = [
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("texture:pressure_0"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                view_formats: &[],
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::COPY_DST,
            }),
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("texture:pressure_1"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                view_formats: &[],
                usage: wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST,
            }),
        ];

        // queue.write_texture(
        //     wgpu::ImageCopyTexture {
        //         texture: &advection_forward_texture,
        //         mip_level: 0,
        //         origin: wgpu::Origin3d::ZERO,
        //         aspect: wgpu::TextureAspect::All,
        //     },
        //     // TODO: remove debugging values
        //     bytemuck::cast_slice(&vec![0.0f32; (2 * width * height) as usize]),
        //     wgpu::ImageDataLayout {
        //         offset: 0,
        //         bytes_per_row: Some(2 * 4 * width),
        //         rows_per_image: Some(height),
        //     },
        //     size,
        // );

        // Texture views

        let velocity_texture_views = [
            velocity_textures[0].create_view(&wgpu::TextureViewDescriptor {
                label: Some("view:velocity_0"),
                ..Default::default()
            }),
            velocity_textures[1].create_view(&wgpu::TextureViewDescriptor {
                label: Some("view:velocity_1"),
                ..Default::default()
            }),
        ];

        let advection_forward_texture_view =
            advection_forward_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("view:advection_forward"),
                ..Default::default()
            });

        let advection_reverse_texture_view =
            advection_reverse_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("view:advection_reverse"),
                ..Default::default()
            });

        let divergence_texture_view =
            divergence_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("view:divergence"),
                ..Default::default()
            });

        let pressure_texture_views = [
            pressure_textures[0].create_view(&wgpu::TextureViewDescriptor {
                label: Some("view:pressure_0"),
                ..Default::default()
            }),
            pressure_textures[1].create_view(&wgpu::TextureViewDescriptor {
                label: Some("view:pressure_1"),
                ..Default::default()
            }),
        ];

        // Samplers

        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sampler:linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Bind group layouts

        let clear_pressure_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Clear pressure bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                }],
            });

        let clear_pressure_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Clear pressure layout"),
                bind_group_layouts: &[&clear_pressure_bind_group_layout],
                push_constant_ranges: &[],
            });

        let clear_pressure_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Clear pressure shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../shader/clear_pressure.wgsl"
            ))),
        });

        let clear_pressure_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Clear pressure"),
                layout: Some(&clear_pressure_pipeline_layout),
                module: &clear_pressure_shader,
                entry_point: "main",
            });

        let velocity_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Velocity bind group layout"),
                entries: &[
                    // velocity_texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // out_velocity_texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rg32Float,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });

        let velocity_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bind_group:velocity_0"),
                layout: &velocity_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&velocity_texture_views[0]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&velocity_texture_views[1]),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bind_group:velocity_1"),
                layout: &velocity_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&velocity_texture_views[1]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&velocity_texture_views[0]),
                    },
                ],
            }),
        ];

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bind_group_layout:uniform"),
                entries: &[
                    // fluid_uniforms
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // linear_sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let advection_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bind_group_layout:uniform"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // out_texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rg32Float,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind group:uniform"),
            layout: &uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &fluid_uniform_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&linear_sampler),
                },
            ],
        });

        let advection_forward_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group:advection_forward"),
            layout: &advection_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&velocity_texture_views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&advection_forward_texture_view),
                },
            ],
        });

        let advection_reverse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group:advection_reverse"),
            layout: &advection_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&velocity_texture_views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&advection_reverse_texture_view),
                },
            ],
        });

        let advection_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Advection layout"),
                bind_group_layouts: &[&uniform_bind_group_layout, &advection_bind_group_layout],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::COMPUTE,
                    range: 0..8,
                }],
            });

        let advection_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader:advection"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../shader/advect.wgsl"
            ))),
        });

        let advection_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Advection"),
            layout: Some(&advection_pipeline_layout),
            module: &advection_shader,
            entry_point: "main",
        });

        let adjust_advection_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bind_group_layout:adjust_advection"),
                entries: &[
                    // forward_advected_texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // reverse_advected_texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let adjust_advection_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group:adjust_advection"),
            layout: &adjust_advection_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&advection_forward_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&advection_reverse_texture_view),
                },
            ],
        });

        let adjust_advection_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipeline_layout:adjust_advection"),
                bind_group_layouts: &[
                    &uniform_bind_group_layout,
                    &adjust_advection_bind_group_layout,
                    &velocity_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let adjust_advection_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader:adjust_advection"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../shader/adjust_advection.comp.wgsl"
            ))),
        });

        let adjust_advection_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("pipeline:adjust_advection"),
                layout: Some(&adjust_advection_pipeline_layout),
                module: &adjust_advection_shader,
                entry_point: "main",
            });

        let diffusion_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader:diffusion"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../shader/diffuse.wgsl"
            ))),
        });

        let diffusion_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Diffusion"),
            layout: Some(&advection_pipeline_layout),
            module: &diffusion_shader,
            entry_point: "main",
        });

        let divergence_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bind_group_layout:divergence"),
                entries: &[
                    // linear_sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // out_divergence_texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });

        let divergence_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group:divergence"),
            layout: &divergence_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&linear_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&divergence_texture_view),
                },
            ],
        });

        let divergence_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipeline_layout:divergence"),
                bind_group_layouts: &[&&divergence_bind_group_layout, &velocity_bind_group_layout],
                push_constant_ranges: &[],
            });

        let divergence_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader:divergence"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../shader/divergence.comp.wgsl"
            ))),
        });

        let divergence_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("pipeline:divergence"),
                layout: Some(&divergence_pipeline_layout),
                module: &divergence_shader,
                entry_point: "main",
            });

        let divergence_sample_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bind_group_layout:divergence_sample"),
                entries: &[
                    // divergence_texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let divergence_sample_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group:divergence_sample"),
            layout: &divergence_sample_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&divergence_texture_view),
            }],
        });

        let pressure_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bind_group_layout:pressure"),
                entries: &[
                    // pressure_texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // out_pressure_texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });

        let pressure_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bind_group:pressure_0"),
                layout: &pressure_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&pressure_texture_views[0]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&pressure_texture_views[1]),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bind_group:pressure_1"),
                layout: &pressure_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&pressure_texture_views[1]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&pressure_texture_views[0]),
                    },
                ],
            }),
        ];

        let pressure_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader:pressure"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../shader/solve_pressure.comp.wgsl"
            ))),
        });

        let pressure_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipeline_layout:pressure"),
                bind_group_layouts: &[
                    &uniform_bind_group_layout,
                    &divergence_sample_bind_group_layout,
                    &pressure_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let pressure_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("pipeline:pressure"),
            layout: Some(&pressure_pipeline_layout),
            module: &pressure_shader,
            entry_point: "main",
        });

        let subtract_gradient_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader:subtract_gradient"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../shader/subtract_gradient.comp.wgsl"
            ))),
        });

        let subtract_gradient_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipeline_layout:subtract_gradient"),
                bind_group_layouts: &[
                    &uniform_bind_group_layout,
                    &pressure_bind_group_layout,
                    &velocity_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let subtract_gradient_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("pipeline:subtract_gradient"),
                layout: Some(&subtract_gradient_pipeline_layout),
                module: &subtract_gradient_shader,
                entry_point: "main",
            });

        Self {
            fluid_size: [width as f32, height as f32],
            fluid_size_3d: size,

            pressure_mode: settings.pressure_mode,
            pressure_iterations: settings.pressure_iterations,

            fluid_uniforms,
            fluid_uniform_buffer,

            velocity_textures,
            velocity_texture_views,
            advection_forward_texture,
            advection_forward_texture_view,
            advection_reverse_texture,
            advection_reverse_texture_view,
            divergence_texture,
            pressure_textures,

            velocity_bind_groups,
            uniform_bind_group,
            advection_forward_bind_group,
            advection_reverse_bind_group,
            adjust_advection_bind_group,
            divergence_bind_group,
            divergence_sample_bind_group,
            pressure_bind_groups,

            clear_pressure_pipeline,
            advection_pipeline,
            adjust_advection_pipeline,
            diffusion_pipeline,
            divergence_pipeline,
            pressure_pipeline,
            subtract_gradient_pipeline,
        }
    }

    fn get_workgroup_size(&self) -> (u32, u32, u32) {
        let [width, height] = self.fluid_size;
        ((width / 8.0).ceil() as u32, (height / 8.0).ceil() as u32, 1)
    }

    pub fn advect_forward<'cpass>(&'cpass self, cpass: &mut wgpu::ComputePass<'cpass>) {
        let workgroup = self.get_workgroup_size();
        cpass.set_pipeline(&self.advection_pipeline);
        cpass.set_push_constants(0, bytemuck::cast_slice(&[1.0]));
        cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        cpass.set_bind_group(1, &self.advection_forward_bind_group, &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
    }

    pub fn advect_reverse<'cpass>(&'cpass self, cpass: &mut wgpu::ComputePass<'cpass>) {
        let workgroup = self.get_workgroup_size();
        cpass.set_pipeline(&self.advection_pipeline);
        cpass.set_push_constants(0, bytemuck::cast_slice(&[-1.0]));
        cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        cpass.set_bind_group(1, &self.advection_reverse_bind_group, &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
    }

    pub fn adjust_advection<'cpass>(&'cpass self, cpass: &mut wgpu::ComputePass<'cpass>) {
        let workgroup = self.get_workgroup_size();
        cpass.set_pipeline(&self.adjust_advection_pipeline);
        cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        cpass.set_bind_group(1, &self.adjust_advection_bind_group, &[]);
        cpass.set_bind_group(2, &self.velocity_bind_groups[0], &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
    }

    pub fn diffuse<'cpass>(&'cpass self, cpass: &mut wgpu::ComputePass<'cpass>) {
        let workgroup = self.get_workgroup_size();
        cpass.set_pipeline(&self.diffusion_pipeline);
        cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        cpass.set_bind_group(1, &self.velocity_bind_groups[1], &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
    }

    pub fn calculate_divergence<'cpass>(&'cpass self, cpass: &mut wgpu::ComputePass<'cpass>) {
        let workgroup = self.get_workgroup_size();
        cpass.set_pipeline(&self.divergence_pipeline);
        cpass.set_bind_group(0, &self.divergence_bind_group, &[]);
        cpass.set_bind_group(1, &self.velocity_bind_groups[1], &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
    }

    pub fn clear_pressure(&self, queue: &wgpu::Queue, pressure: f32) {
        let (width, height) = (self.fluid_size[0] as u32, self.fluid_size[1] as u32);

        for pressure_texture in self.pressure_textures.iter() {
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &pressure_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                bytemuck::cast_slice(&vec![pressure; (width * height) as usize]),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width),
                    rows_per_image: Some(height),
                },
                self.fluid_size_3d,
            );
        }
    }

    pub fn solve_pressure<'cpass>(
        &'cpass self,
        queue: &wgpu::Queue,
        cpass: &mut wgpu::ComputePass<'cpass>,
    ) {
        use settings::PressureMode::*;
        match self.pressure_mode {
            ClearWith(pressure) => {
                self.clear_pressure(queue, pressure);
            }
            Retain => (),
        }

        let workgroup = self.get_workgroup_size();
        cpass.set_pipeline(&self.pressure_pipeline);
        cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        cpass.set_bind_group(1, &self.divergence_sample_bind_group, &[]);

        let mut index = 0;

        for _ in 0..self.pressure_iterations {
            cpass.set_bind_group(2, &self.pressure_bind_groups[index], &[]);
            cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
            index = 1 - index;
        }
    }

    pub fn subtract_gradient<'cpass>(&'cpass self, cpass: &mut wgpu::ComputePass<'cpass>) {
        let workgroup = self.get_workgroup_size();
        cpass.set_pipeline(&self.subtract_gradient_pipeline);
        cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        cpass.set_bind_group(1, &self.pressure_bind_groups[0], &[]); // TODO: get correct index
        cpass.set_bind_group(2, &self.velocity_bind_groups[1], &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
    }

    pub fn get_fluid_size(&self) -> wgpu::Extent3d {
        self.fluid_size_3d
    }

    // TODO: fix texture
    pub fn get_velocity_texture_view(&self) -> &wgpu::TextureView {
        &self.velocity_texture_views[0]
    }

    pub fn get_advection_forward_texture_view(&self) -> &wgpu::TextureView {
        &self.advection_forward_texture_view
    }

    pub fn get_velocity_bind_group(&self, index: usize) -> &wgpu::BindGroup {
        &self.velocity_bind_groups[index]
    }
}
