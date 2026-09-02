use crate::grid;
use crate::settings::{self, Settings};
use crate::BackendCaps;

use super::downgrade_float_storage;

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
    pub fn new(settings: &Settings) -> Self {
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

/// The bind group layouts that the fluid textures are bound with. They outlive
/// any one set of textures.
struct Layouts {
    velocity: wgpu::BindGroupLayout,
    advection: wgpu::BindGroupLayout,
    adjust_advection: wgpu::BindGroupLayout,
    divergence: wgpu::BindGroupLayout,
    divergence_sample: wgpu::BindGroupLayout,
    pressure: wgpu::BindGroupLayout,
}

/// The textures that hold the simulation, and the bind groups that reference
/// them. The fluid rebuilds this as a unit when it changes size. A texture view
/// keeps its texture alive, so only the pressure textures are stored by
/// handle; `clear_pressure` writes to them directly.
struct Field {
    size: wgpu::Extent3d,

    velocity_texture_views: [wgpu::TextureView; 2],
    advection_forward_texture_view: wgpu::TextureView,
    divergence_texture_view: wgpu::TextureView,
    pressure_textures: [wgpu::Texture; 2],
    pressure_texture_views: [wgpu::TextureView; 2],

    velocity_bind_groups: [wgpu::BindGroup; 2],
    advection_forward_bind_group: wgpu::BindGroup,
    advection_reverse_bind_group: wgpu::BindGroup,
    advection_reverse_input_bind_group: wgpu::BindGroup,
    adjust_advection_bind_group: wgpu::BindGroup,
    divergence_bind_group: wgpu::BindGroup,
    divergence_sample_bind_group: wgpu::BindGroup,
    pressure_bind_groups: [wgpu::BindGroup; 2],
}

impl Field {
    fn new(
        device: &wgpu::Device,
        size: wgpu::Extent3d,
        pressure_format: wgpu::TextureFormat,
        layouts: &Layouts,
        nearest_sampler: &wgpu::Sampler,
    ) -> Self {
        let create_texture = |label: &str, format: wgpu::TextureFormat| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("texture:{label}")),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                view_formats: &[],
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::COPY_DST,
            })
        };
        let create_view = |label: &str, texture: &wgpu::Texture| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(&format!("view:{label}")),
                ..Default::default()
            })
        };

        // Textures

        let velocity_textures = [
            create_texture("velocity_0", wgpu::TextureFormat::Rgba16Float),
            create_texture("velocity_1", wgpu::TextureFormat::Rgba16Float),
        ];
        let advection_forward_texture =
            create_texture("advection_forward", wgpu::TextureFormat::Rgba16Float);
        let advection_reverse_texture =
            create_texture("advection_reverse", wgpu::TextureFormat::Rgba16Float);
        let divergence_texture = create_texture("divergence", wgpu::TextureFormat::R32Float);
        let pressure_textures = [
            create_texture("pressure_0", pressure_format),
            create_texture("pressure_1", pressure_format),
        ];

        // Texture views

        let velocity_texture_views = [
            create_view("velocity_0", &velocity_textures[0]),
            create_view("velocity_1", &velocity_textures[1]),
        ];
        let advection_forward_texture_view =
            create_view("advection_forward", &advection_forward_texture);
        let advection_reverse_texture_view =
            create_view("advection_reverse", &advection_reverse_texture);
        let divergence_texture_view = create_view("divergence", &divergence_texture);
        let pressure_texture_views = [
            create_view("pressure_0", &pressure_textures[0]),
            create_view("pressure_1", &pressure_textures[1]),
        ];

        // Bind groups

        let velocity_bind_group =
            |label: &str, input: &wgpu::TextureView, output: &wgpu::TextureView| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("bind_group:{label}")),
                    layout: &layouts.velocity,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(input),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(output),
                        },
                    ],
                })
            };

        let velocity_bind_groups = [
            velocity_bind_group(
                "velocity_0",
                &velocity_texture_views[0],
                &velocity_texture_views[1],
            ),
            velocity_bind_group(
                "velocity_1",
                &velocity_texture_views[1],
                &velocity_texture_views[0],
            ),
        ];

        // For the reverse advection pass (MacCormack step 2), the input is the
        // forward-advected texture, not the original velocity.
        let advection_reverse_input_bind_group = velocity_bind_group(
            "advection_reverse_input",
            &advection_forward_texture_view,
            &velocity_texture_views[0],
        );

        let advection_bind_group = |label: &str, output: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("bind_group:{label}")),
                layout: &layouts.advection,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(output),
                }],
            })
        };

        let advection_forward_bind_group =
            advection_bind_group("advection_forward", &advection_forward_texture_view);
        let advection_reverse_bind_group =
            advection_bind_group("advection_reverse", &advection_reverse_texture_view);

        let adjust_advection_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group:adjust_advection"),
            layout: &layouts.adjust_advection,
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

        let divergence_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group:divergence"),
            layout: &layouts.divergence,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(nearest_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&divergence_texture_view),
                },
            ],
        });

        let divergence_sample_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group:divergence_sample"),
            layout: &layouts.divergence_sample,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&divergence_texture_view),
            }],
        });

        let pressure_bind_group =
            |label: &str, input: &wgpu::TextureView, output: &wgpu::TextureView| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("bind_group:{label}")),
                    layout: &layouts.pressure,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(input),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(output),
                        },
                    ],
                })
            };

        let pressure_bind_groups = [
            pressure_bind_group(
                "pressure_0",
                &pressure_texture_views[0],
                &pressure_texture_views[1],
            ),
            pressure_bind_group(
                "pressure_1",
                &pressure_texture_views[1],
                &pressure_texture_views[0],
            ),
        ];

        Self {
            size,

            velocity_texture_views,
            advection_forward_texture_view,
            divergence_texture_view,
            pressure_textures,
            pressure_texture_views,

            velocity_bind_groups,
            advection_forward_bind_group,
            advection_reverse_bind_group,
            advection_reverse_input_bind_group,
            adjust_advection_bind_group,
            divergence_bind_group,
            divergence_sample_bind_group,
            pressure_bind_groups,
        }
    }

    fn workgroup_count(&self) -> (u32, u32, u32) {
        (
            self.size.width.div_ceil(16),
            self.size.height.div_ceil(16),
            1,
        )
    }
}

pub struct Context {
    diffusion_iterations: u32,
    pressure_mode: settings::PressureMode,
    pressure_iterations: u32,

    fluid_uniforms: FluidUniforms,
    fluid_uniform_buffer: wgpu::Buffer,

    pressure_format: wgpu::TextureFormat,
    linear_sampler: wgpu::Sampler,
    nearest_sampler: wgpu::Sampler,
    layouts: Layouts,
    field: Field,

    uniform_bind_group: wgpu::BindGroup,
    advection_forward_direction_bind_group: wgpu::BindGroup,
    advection_reverse_direction_bind_group: wgpu::BindGroup,

    advection_pipeline: wgpu::ComputePipeline,
    adjust_advection_pipeline: wgpu::ComputePipeline,
    diffusion_pipeline: wgpu::ComputePipeline,
    divergence_pipeline: wgpu::ComputePipeline,
    pressure_pipeline: wgpu::ComputePipeline,
    subtract_gradient_pipeline: wgpu::ComputePipeline,

    resample_bind_group_layout: wgpu::BindGroupLayout,
    resample_pipeline: wgpu::ComputePipeline,

    last_pressure_index: Arc<Mutex<usize>>,
    last_velocity_index: Arc<Mutex<usize>>,
}

impl Context {
    pub fn update(&mut self, queue: &wgpu::Queue, settings: &Arc<Settings>) {
        // Update fluid settings needed on the CPU side
        self.diffusion_iterations = settings.diffusion_iterations;
        self.pressure_mode = settings.pressure_mode;
        self.pressure_iterations = settings.pressure_iterations;

        // Update uniforms
        self.fluid_uniforms = FluidUniforms::new(settings);
        queue.write_buffer(
            &self.fluid_uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.fluid_uniforms]),
        );
    }

    /// Follow the grid to a new size. The fluid keeps the aspect ratio of the
    /// grid so that the simulation is isotropic on screen. Nothing happens if
    /// the size is unchanged.
    ///
    /// The velocity field is resampled into the new textures, so the flow
    /// continues where it was. Pressure starts from zero and settles within a
    /// few solver iterations.
    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scaling_ratio: grid::ScalingRatio,
        settings: &Settings,
    ) {
        let size = scaling_ratio.texture_size(settings.fluid_size);
        if size == self.field.size {
            return;
        }

        let field = Field::new(
            device,
            size,
            self.pressure_format,
            &self.layouts,
            &self.nearest_sampler,
        );

        let velocity_index = *self.last_velocity_index.lock().unwrap();
        let resample_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group:resample_velocity"),
            layout: &self.resample_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &self.field.velocity_texture_views[velocity_index],
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&field.velocity_texture_views[0]),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("encoder:resample_velocity"),
        });
        {
            let workgroup = field.workgroup_count();
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("flux::resample_velocity"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.resample_pipeline);
            cpass.set_bind_group(0, &resample_bind_group, &[]);
            cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
        }
        queue.submit(Some(encoder.finish()));

        self.field = field;
        *self.last_velocity_index.lock().unwrap() = 0;
        *self.last_pressure_index.lock().unwrap() = 0;
    }

    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        scaling_ratio: grid::ScalingRatio,
        caps: BackendCaps,
        settings: &Arc<Settings>,
    ) -> Self {
        let size = scaling_ratio.texture_size(settings.fluid_size);

        // Pressure is linearly sampled in `subtract_gradient.comp.wgsl`, which
        // requires `FLOAT32_FILTERABLE` for an R32Float texture. The shaders
        // only touch the `.x` channel, so widening the fallback to Rgba16Float
        // is semantically a no-op — see `downgrade_float_storage` for why we
        // can't fall back to the narrower R16Float.
        let pressure_format = if caps.float32_filterable {
            wgpu::TextureFormat::R32Float
        } else {
            wgpu::TextureFormat::Rgba16Float
        };

        // Uniforms

        let fluid_uniforms = FluidUniforms::new(settings);
        let fluid_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniform:FluidUniforms"),
            contents: bytemuck::cast_slice(&[fluid_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

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
                            format: wgpu::TextureFormat::Rgba16Float,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });

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
                label: Some("bind_group_layout:advection"),
                entries: &[
                    // out_texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rgba16Float,
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

        let divergence_sample_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bind_group_layout:divergence_sample"),
                entries: &[
                    // divergence_texture — read with textureLoad in solve_pressure.comp.wgsl,
                    // never linearly sampled, so the texture format does not need to be
                    // filterable. Declaring `filterable: false` lets the divergence
                    // texture stay R32Float even when FLOAT32_FILTERABLE is unavailable.
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
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
                            format: pressure_format,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });

        let resample_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bind_group_layout:resample_velocity"),
                entries: &[
                    // linear_sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // velocity_texture
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
                    // out_velocity_texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rgba16Float,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });

        let layouts = Layouts {
            velocity: velocity_bind_group_layout,
            advection: advection_bind_group_layout,
            adjust_advection: adjust_advection_bind_group_layout,
            divergence: divergence_bind_group_layout,
            divergence_sample: divergence_sample_bind_group_layout,
            pressure: pressure_bind_group_layout,
        };

        // Bind groups that outlive the textures

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

        let direction_bind_group = |label: &str, direction: f32| {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("uniform:{label}")),
                contents: bytemuck::cast_slice(&[Direction {
                    _padding: [0; 3],
                    direction,
                }]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("bind_group:advection_{label}_direction")),
                layout: &advection_direction_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &buffer,
                        offset: 0,
                        size: None,
                    }),
                }],
            })
        };

        let advection_forward_direction_bind_group = direction_bind_group("forward", 1.0);
        let advection_reverse_direction_bind_group = direction_bind_group("reverse", -1.0);

        // Pipelines

        let advection_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Advection layout"),
                bind_group_layouts: &[
                    Some(&uniform_bind_group_layout),
                    Some(&layouts.advection),
                    Some(&advection_direction_bind_group_layout),
                    Some(&layouts.velocity),
                ],
                immediate_size: 0,
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

        let adjust_advection_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipeline_layout:adjust_advection"),
                bind_group_layouts: &[
                    Some(&uniform_bind_group_layout),
                    Some(&layouts.adjust_advection),
                    Some(&layouts.velocity),
                ],
                immediate_size: 0,
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
                bind_group_layouts: &[Some(&uniform_bind_group_layout), Some(&layouts.velocity)],
                immediate_size: 0,
            });

        let diffusion_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Diffusion"),
            layout: Some(&diffusion_pipeline_layout),
            module: &diffusion_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let divergence_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipeline_layout:divergence"),
                bind_group_layouts: &[Some(&layouts.divergence), Some(&layouts.velocity)],
                immediate_size: 0,
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

        let pressure_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader:pressure"),
            source: wgpu::ShaderSource::Wgsl(downgrade_float_storage(
                include_str!("../../shader/solve_pressure.comp.wgsl"),
                caps,
            )),
        });

        let pressure_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipeline_layout:pressure"),
                bind_group_layouts: &[
                    Some(&uniform_bind_group_layout),
                    Some(&layouts.divergence_sample),
                    Some(&layouts.pressure),
                ],
                immediate_size: 0,
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
                    Some(&uniform_bind_group_layout),
                    Some(&layouts.pressure),
                    Some(&layouts.velocity),
                ],
                immediate_size: 0,
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

        let resample_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader:resample_velocity"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../shader/resample_velocity.comp.wgsl"
            ))),
        });

        let resample_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipeline_layout:resample_velocity"),
                bind_group_layouts: &[Some(&resample_bind_group_layout)],
                immediate_size: 0,
            });

        let resample_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("pipeline:resample_velocity"),
            layout: Some(&resample_pipeline_layout),
            module: &resample_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let field = Field::new(device, size, pressure_format, &layouts, &nearest_sampler);

        Self {
            diffusion_iterations: settings.diffusion_iterations,
            pressure_mode: settings.pressure_mode,
            pressure_iterations: settings.pressure_iterations,

            fluid_uniforms,
            fluid_uniform_buffer,

            pressure_format,
            linear_sampler,
            nearest_sampler,
            layouts,
            field,

            uniform_bind_group,
            advection_forward_direction_bind_group,
            advection_reverse_direction_bind_group,

            advection_pipeline,
            adjust_advection_pipeline,
            diffusion_pipeline,
            divergence_pipeline,
            pressure_pipeline,
            subtract_gradient_pipeline,

            resample_bind_group_layout,
            resample_pipeline,

            last_pressure_index: Arc::new(Mutex::new(0)),
            last_velocity_index: Arc::new(Mutex::new(0)),
        }
    }

    pub fn advect_forward<'cpass>(
        &'cpass self,
        _queue: &wgpu::Queue,
        cpass: &mut wgpu::ComputePass<'cpass>,
    ) {
        let velocity_index = self.last_velocity_index.lock().unwrap();
        let workgroup = self.field.workgroup_count();
        cpass.set_pipeline(&self.advection_pipeline);
        cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        cpass.set_bind_group(1, &self.field.advection_forward_bind_group, &[]);
        cpass.set_bind_group(2, &self.advection_forward_direction_bind_group, &[]);
        cpass.set_bind_group(3, &self.field.velocity_bind_groups[*velocity_index], &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
    }

    pub fn advect_reverse<'cpass>(
        &'cpass self,
        _queue: &wgpu::Queue,
        cpass: &mut wgpu::ComputePass<'cpass>,
    ) {
        let workgroup = self.field.workgroup_count();
        cpass.set_pipeline(&self.advection_pipeline);
        cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        cpass.set_bind_group(1, &self.field.advection_reverse_bind_group, &[]);
        cpass.set_bind_group(2, &self.advection_reverse_direction_bind_group, &[]);
        // MacCormack step 2: re-advect the forward-advected result (not the original velocity)
        cpass.set_bind_group(3, &self.field.advection_reverse_input_bind_group, &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
    }

    pub fn adjust_advection<'cpass>(&'cpass self, cpass: &mut wgpu::ComputePass<'cpass>) {
        let mut velocity_index = self.last_velocity_index.lock().unwrap();
        let workgroup = self.field.workgroup_count();
        cpass.set_pipeline(&self.adjust_advection_pipeline);
        cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        cpass.set_bind_group(1, &self.field.adjust_advection_bind_group, &[]);
        cpass.set_bind_group(2, &self.field.velocity_bind_groups[*velocity_index], &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);

        *velocity_index = 1 - *velocity_index;
    }

    pub fn diffuse<'cpass>(&'cpass self, cpass: &mut wgpu::ComputePass<'cpass>) {
        let mut velocity_index = self.last_velocity_index.lock().unwrap();
        let workgroup = self.field.workgroup_count();
        cpass.set_pipeline(&self.diffusion_pipeline);
        cpass.set_bind_group(0, &self.uniform_bind_group, &[]);

        for _ in 0..self.diffusion_iterations {
            cpass.set_bind_group(1, &self.field.velocity_bind_groups[*velocity_index], &[]);
            cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
            *velocity_index = 1 - *velocity_index;
        }
    }

    pub fn calculate_divergence<'cpass>(&'cpass self, cpass: &mut wgpu::ComputePass<'cpass>) {
        let velocity_index = self.last_velocity_index.lock().unwrap();
        let workgroup = self.field.workgroup_count();
        cpass.set_pipeline(&self.divergence_pipeline);
        cpass.set_bind_group(0, &self.field.divergence_bind_group, &[]);
        cpass.set_bind_group(1, &self.field.velocity_bind_groups[*velocity_index], &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
    }

    pub fn clear_pressure(&self, queue: &wgpu::Queue, pressure: f32) {
        let size = self.field.size;
        let pixel_count = (size.width * size.height) as usize;

        // The pressure texture format depends on FLOAT32_FILTERABLE support:
        // R32Float on the fast path, Rgba16Float on the fallback. Build the
        // upload buffer from whichever the actual texture is using.
        let format = self.pressure_format;
        let bytes_per_pixel = format
            .block_copy_size(None)
            .expect("pressure format is uncompressed and color-only");
        let buf: Vec<u8> = match format {
            wgpu::TextureFormat::R32Float => {
                bytemuck::cast_slice(&vec![pressure; pixel_count]).to_vec()
            }
            wgpu::TextureFormat::Rgba16Float => {
                let p = half::f16::from_f32(pressure);
                let texel = [p, half::f16::ZERO, half::f16::ZERO, half::f16::ZERO];
                bytemuck::cast_slice(&vec![texel; pixel_count]).to_vec()
            }
            other => panic!("unexpected pressure format: {other:?}"),
        };

        for pressure_texture in self.field.pressure_textures.iter() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: pressure_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &buf,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_pixel * size.width),
                    rows_per_image: Some(size.height),
                },
                size,
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
        let workgroup = self.field.workgroup_count();
        cpass.set_pipeline(&self.pressure_pipeline);
        cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        cpass.set_bind_group(1, &self.field.divergence_sample_bind_group, &[]);

        for _ in 0..self.pressure_iterations {
            cpass.set_bind_group(2, &self.field.pressure_bind_groups[*pressure_index], &[]);
            cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
            *pressure_index = 1 - *pressure_index;
        }
    }

    pub fn subtract_gradient<'cpass>(&'cpass self, cpass: &mut wgpu::ComputePass<'cpass>) {
        let pressure_index = self.last_pressure_index.lock().unwrap();
        let mut velocity_index = self.last_velocity_index.lock().unwrap();
        let workgroup = self.field.workgroup_count();
        cpass.set_pipeline(&self.subtract_gradient_pipeline);
        cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        cpass.set_bind_group(1, &self.field.pressure_bind_groups[*pressure_index], &[]);
        cpass.set_bind_group(2, &self.field.velocity_bind_groups[*velocity_index], &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
        *velocity_index = 1 - *velocity_index;
    }

    pub fn get_fluid_size(&self) -> wgpu::Extent3d {
        self.field.size
    }

    pub fn get_velocity_texture_view(&self) -> &wgpu::TextureView {
        let index = self.last_velocity_index.lock().unwrap();
        &self.field.velocity_texture_views[*index]
    }

    pub fn get_advection_forward_texture_view(&self) -> &wgpu::TextureView {
        &self.field.advection_forward_texture_view
    }

    pub fn get_divergence_texture_view(&self) -> &wgpu::TextureView {
        &self.field.divergence_texture_view
    }

    pub fn get_pressure_texture_view(&self) -> &wgpu::TextureView {
        let index = self.last_pressure_index.lock().unwrap();
        &self.field.pressure_texture_views[*index]
    }

    pub fn get_read_velocity_bind_group(&self) -> &wgpu::BindGroup {
        let index = self.last_velocity_index.lock().unwrap();
        &self.field.velocity_bind_groups[*index]
    }

    pub fn get_write_velocity_bind_group(&self) -> &wgpu::BindGroup {
        let mut index = self.last_velocity_index.lock().unwrap();
        let curr_index = *index;
        *index = 1 - *index;
        &self.field.velocity_bind_groups[curr_index]
    }
}
