use crate::grid;
use crate::settings::{self, Settings};

use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Direction {
    _padding: [u32; 3],
    pub direction: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct FluidUniforms {
    timestep: f32,       // 0
    dissipation: f32,    // 4
    alpha: f32,          // 8
    r_beta: f32,         // 12
    center_factor: f32,  // 16
    stencil_factor: f32, // 20
    _padding0: u32,      // 24
    _padding1: u32,      // 28
                         // roundUp(4, 24) = 24 -> roundUp to 32
}

impl FluidUniforms {
    pub fn new(size: &wgpu::Extent3d, settings: &Settings) -> Self {
        // dx^2 / (rho * dt)
        let center_factor = 1.0 / (settings.viscosity * settings.fluid_timestep);
        let stencil_factor = 1.0 / (4.0 + center_factor);

        FluidUniforms {
            timestep: settings.fluid_timestep,
            dissipation: settings.velocity_dissipation,
            alpha: -1.0,
            r_beta: 0.25,
            center_factor,
            stencil_factor,
            _padding0: 0,
            _padding1: 0,
        }
    }
}

pub struct Context {
    fluid_size: [f32; 2],
    fluid_size_3d: wgpu::Extent3d,

    diffusion_iterations: u32,
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
    divergence_texture_view: wgpu::TextureView,
    pressure_textures: [wgpu::Texture; 2],
    pressure_texture_views: [wgpu::TextureView; 2],

    velocity_bind_groups: [wgpu::BindGroup; 2],
    uniform_bind_group: wgpu::BindGroup,
    advection_forward_bind_group: wgpu::BindGroup,
    advection_reverse_bind_group: wgpu::BindGroup,
    advection_forward_direction_bind_group: wgpu::BindGroup,
    advection_reverse_direction_bind_group: wgpu::BindGroup,
    adjust_advection_bind_group: wgpu::BindGroup,
    divergence_bind_group: wgpu::BindGroup,
    divergence_sample_bind_group: wgpu::BindGroup,
    pressure_bind_groups: [wgpu::BindGroup; 2],

    advection_pipeline: wgpu::ComputePipeline,
    adjust_advection_pipeline: wgpu::ComputePipeline,
    diffusion_pipeline: wgpu::ComputePipeline,
    divergence_pipeline: wgpu::ComputePipeline,
    pressure_pipeline: wgpu::ComputePipeline,
    subtract_gradient_pipeline: wgpu::ComputePipeline,

    last_pressure_index: Arc<Mutex<usize>>,
    last_velocity_index: Arc<Mutex<usize>>,
}

impl Context {
    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scaling_ratio: grid::ScalingRatio,
        settings: &Arc<Settings>,
    ) {
        let (width, height) = (
            scaling_ratio.rounded_x() * settings.fluid_size,
            scaling_ratio.rounded_y() * settings.fluid_size,
        );
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        // Resize the fluid texture if necessary
        if self.fluid_size_3d != size {
            self.fluid_size = [width as f32, height as f32];
            self.fluid_size_3d = size;
            // self.resize_fluid_texture(width, height).unwrap();
        }

        // Update fluid settings needed on the CPU side
        self.diffusion_iterations = settings.diffusion_iterations;
        self.pressure_mode = settings.pressure_mode;
        self.pressure_iterations = settings.pressure_iterations;

        // Update uniforms
        self.fluid_uniforms = FluidUniforms::new(&size, settings);
        queue.write_buffer(
            &self.fluid_uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.fluid_uniforms]),
        );
    }

    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scaling_ratio: grid::ScalingRatio,
        settings: &Arc<Settings>,
    ) -> Self {
        let (width, height) = (
            scaling_ratio.rounded_x() * settings.fluid_size,
            scaling_ratio.rounded_y() * settings.fluid_size,
        );
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        // Uniforms

        let fluid_uniforms = FluidUniforms::new(&size, settings);
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
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sampler:nearest"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        // Bind group layouts

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
                    // nearest_sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
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
                    // out_texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
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

        let advection_direction_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bind_group_layout:advection_direction"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
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
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&nearest_sampler),
                },
            ],
        });

        let forward_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniform:forward"),
            contents: bytemuck::cast_slice(&[Direction {
                _padding: [0; 3],
                direction: 1.0,
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let reverse_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniform:reverse"),
            contents: bytemuck::cast_slice(&[Direction {
                _padding: [0; 3],
                direction: -1.0,
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let advection_forward_direction_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bind_group:advection_forward_direction"),
                layout: &advection_direction_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &forward_buffer,
                        offset: 0,
                        size: None,
                    }),
                }],
            });

        let advection_reverse_direction_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bind_group:advection_reverse_direction"),
                layout: &advection_direction_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &reverse_buffer,
                        offset: 0,
                        size: None,
                    }),
                }],
            });

        let advection_forward_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group:advection_forward"),
            layout: &advection_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&advection_forward_texture_view),
            }],
        });

        let advection_reverse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group:advection_reverse"),
            layout: &advection_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&advection_reverse_texture_view),
            }],
        });

        let advection_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Advection layout"),
                bind_group_layouts: &[
                    &uniform_bind_group_layout,
                    &advection_bind_group_layout,
                    &advection_direction_bind_group_layout,
                    &velocity_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let advection_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader:advection"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../shader/advect.comp.wgsl"
            ))),
        });

        let advection_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Advection"),
            layout: Some(&advection_pipeline_layout),
            module: &advection_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
            // TODO: use pipeline constants for direction once #5500 lands
            // https://github.com/gfx-rs/wgpu/pull/5500
            // constants: HashMap::from([("direction", 1)]),
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
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });

        let diffusion_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader:diffusion"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../shader/diffuse.comp.wgsl"
            ))),
        });

        let diffusion_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipeline_layout:diffusion"),
                bind_group_layouts: &[&uniform_bind_group_layout, &velocity_bind_group_layout],
                push_constant_ranges: &[],
            });

        let diffusion_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Diffusion"),
            layout: Some(&diffusion_pipeline_layout),
            module: &diffusion_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
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
                    resource: wgpu::BindingResource::Sampler(&nearest_sampler),
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
                bind_group_layouts: &[&divergence_bind_group_layout, &velocity_bind_group_layout],
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
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
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
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
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
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });

        Self {
            fluid_size: [width as f32, height as f32],
            fluid_size_3d: size,

            diffusion_iterations: settings.diffusion_iterations,
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
            divergence_texture_view,
            pressure_textures,
            pressure_texture_views,

            velocity_bind_groups,
            uniform_bind_group,
            advection_forward_bind_group,
            advection_reverse_bind_group,
            advection_forward_direction_bind_group,
            advection_reverse_direction_bind_group,
            adjust_advection_bind_group,
            divergence_bind_group,
            divergence_sample_bind_group,
            pressure_bind_groups,

            advection_pipeline,
            adjust_advection_pipeline,
            diffusion_pipeline,
            divergence_pipeline,
            pressure_pipeline,
            subtract_gradient_pipeline,

            last_pressure_index: Arc::new(Mutex::new(0)),
            last_velocity_index: Arc::new(Mutex::new(0)),
        }
    }

    fn get_workgroup_size(&self) -> (u32, u32, u32) {
        let [width, height] = self.fluid_size;
        (
            (width / 16.0).ceil() as u32,
            (height / 16.0).ceil() as u32,
            1,
        )
    }

    pub fn advect_forward<'cpass>(
        &'cpass self,
        _queue: &wgpu::Queue,
        cpass: &mut wgpu::ComputePass<'cpass>,
    ) {
        let velocity_index = self.last_velocity_index.lock().unwrap();
        let workgroup = self.get_workgroup_size();
        cpass.set_pipeline(&self.advection_pipeline);
        cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        cpass.set_bind_group(1, &self.advection_forward_bind_group, &[]);
        cpass.set_bind_group(2, &self.advection_forward_direction_bind_group, &[]);
        cpass.set_bind_group(3, &self.velocity_bind_groups[*velocity_index], &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
    }

    pub fn advect_reverse<'cpass>(
        &'cpass self,
        _queue: &wgpu::Queue,
        cpass: &mut wgpu::ComputePass<'cpass>,
    ) {
        let velocity_index = self.last_velocity_index.lock().unwrap();
        let workgroup = self.get_workgroup_size();
        cpass.set_pipeline(&self.advection_pipeline);
        cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        cpass.set_bind_group(1, &self.advection_reverse_bind_group, &[]);
        cpass.set_bind_group(2, &self.advection_reverse_direction_bind_group, &[]);
        cpass.set_bind_group(3, &self.velocity_bind_groups[*velocity_index], &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
    }

    pub fn adjust_advection<'cpass>(&'cpass self, cpass: &mut wgpu::ComputePass<'cpass>) {
        let mut velocity_index = self.last_velocity_index.lock().unwrap();
        let workgroup = self.get_workgroup_size();
        cpass.set_pipeline(&self.adjust_advection_pipeline);
        cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        cpass.set_bind_group(1, &self.adjust_advection_bind_group, &[]);
        cpass.set_bind_group(2, &self.velocity_bind_groups[*velocity_index], &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);

        *velocity_index = 1 - *velocity_index;
    }

    pub fn diffuse<'cpass>(&'cpass self, cpass: &mut wgpu::ComputePass<'cpass>) {
        let mut velocity_index = self.last_velocity_index.lock().unwrap();
        let workgroup = self.get_workgroup_size();
        cpass.set_pipeline(&self.diffusion_pipeline);
        cpass.set_bind_group(0, &self.uniform_bind_group, &[]);

        for _ in 0..self.diffusion_iterations {
            cpass.set_bind_group(1, &self.velocity_bind_groups[*velocity_index], &[]);
            cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
            *velocity_index = 1 - *velocity_index;
        }
    }

    pub fn calculate_divergence<'cpass>(&'cpass self, cpass: &mut wgpu::ComputePass<'cpass>) {
        let velocity_index = self.last_velocity_index.lock().unwrap();
        let workgroup = self.get_workgroup_size();
        cpass.set_pipeline(&self.divergence_pipeline);
        cpass.set_bind_group(0, &self.divergence_bind_group, &[]);
        cpass.set_bind_group(1, &self.velocity_bind_groups[*velocity_index], &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
    }

    pub fn clear_pressure(&self, queue: &wgpu::Queue, pressure: f32) {
        let (width, height) = (self.fluid_size[0] as u32, self.fluid_size[1] as u32);

        for pressure_texture in self.pressure_textures.iter() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: pressure_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                bytemuck::cast_slice(&vec![pressure; (width * height) as usize]),
                wgpu::TexelCopyBufferLayout {
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

        let mut pressure_index = self.last_pressure_index.lock().unwrap();
        let workgroup = self.get_workgroup_size();
        cpass.set_pipeline(&self.pressure_pipeline);
        cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        cpass.set_bind_group(1, &self.divergence_sample_bind_group, &[]);

        for _ in 0..self.pressure_iterations {
            cpass.set_bind_group(2, &self.pressure_bind_groups[*pressure_index], &[]);
            cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
            *pressure_index = 1 - *pressure_index;
        }
    }

    pub fn subtract_gradient<'cpass>(&'cpass self, cpass: &mut wgpu::ComputePass<'cpass>) {
        let pressure_index = self.last_pressure_index.lock().unwrap();
        let mut velocity_index = self.last_velocity_index.lock().unwrap();
        let workgroup = self.get_workgroup_size();
        cpass.set_pipeline(&self.subtract_gradient_pipeline);
        cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        cpass.set_bind_group(1, &self.pressure_bind_groups[*pressure_index], &[]);
        cpass.set_bind_group(2, &self.velocity_bind_groups[*velocity_index], &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
        *velocity_index = 1 - *velocity_index;
    }

    pub fn get_fluid_size(&self) -> wgpu::Extent3d {
        self.fluid_size_3d
    }

    pub fn get_velocity_texture_view(&self) -> &wgpu::TextureView {
        let index = self.last_velocity_index.lock().unwrap();
        &self.velocity_texture_views[*index]
    }

    pub fn get_advection_forward_texture_view(&self) -> &wgpu::TextureView {
        &self.advection_forward_texture_view
    }

    pub fn get_divergence_texture_view(&self) -> &wgpu::TextureView {
        &self.divergence_texture_view
    }

    pub fn get_pressure_texture_view(&self) -> &wgpu::TextureView {
        let index = self.last_pressure_index.lock().unwrap();
        &self.pressure_texture_views[*index]
    }

    pub fn get_read_velocity_bind_group(&self) -> &wgpu::BindGroup {
        let index = self.last_velocity_index.lock().unwrap();
        &self.velocity_bind_groups[*index]
    }

    pub fn get_write_velocity_bind_group(&self) -> &wgpu::BindGroup {
        let mut index = self.last_velocity_index.lock().unwrap();
        let curr_index = *index;
        *index = 1 - *index;
        &self.velocity_bind_groups[curr_index]
    }
}
