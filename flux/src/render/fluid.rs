use crate::grid;
use crate::settings::{self, Settings};
use crate::BackendCaps;

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
    timestep: f32,
    dissipation: f32,
    // Velocity is in fixed units of the tuned reference simulation (1/128 per axis).
    inverse_cell: [f32; 2],
    velocity_to_uv: [f32; 2],
    diffusion_weight: [f32; 2],
    pressure_clear: f32,
    _padding: [u32; 3],
}

/// Jacobi propagation reaches O(sqrt(iterations)) cells. Refining a field
/// therefore needs quadratic work to retain the same world-space influence.
/// Caps bound worst-case work; extreme quality requests may converge less.
fn solver_iterations(base: u32, refinement: f32, cap: u32) -> u32 {
    if base == 0 {
        return 0;
    }
    ((base as f32 * refinement.powi(2)).ceil() as u32).clamp(1, cap)
}

impl FluidUniforms {
    fn new(settings: &Settings, domain: grid::ScalingRatio, size: wgpu::Extent3d) -> Self {
        let simulation_domain = domain.simulation_domain();
        let inverse_cell = [
            size.width as f32 / (128.0 * simulation_domain[0]),
            size.height as f32 / (128.0 * simulation_domain[1]),
        ];
        // Diffusion samples a fixed reference-world stencil. Refining its
        // texture therefore does not shrink its reach or require repeated
        // sub-ULP rgba16 updates that bias weak diffusion toward zero.
        let vdt = (settings.viscosity * settings.fluid_timestep).max(0.0);
        let diffusivity = vdt / (1.0 + 4.0 * vdt);
        let reference_tap = [1.0_f32, 1.0];
        let diffusion_weight = std::array::from_fn(|axis| {
            let texels = reference_tap[axis] * inverse_cell[axis];
            let fraction = texels.fract();
            // Bilinear sampling adds interpolation variance; compensate it
            // so fractional taps retain the same long-wave diffusivity.
            let variance = texels * texels;
            diffusivity / reference_tap[axis].powi(2) * variance
                / (variance + fraction * (1.0 - fraction))
        });
        Self {
            timestep: settings.fluid_timestep,
            dissipation: settings.velocity_dissipation,
            inverse_cell,
            velocity_to_uv: simulation_domain.map(|axis| 1.0 / (128.0 * axis)),
            diffusion_weight,
            pressure_clear: match settings.pressure_mode {
                settings::PressureMode::ClearWith(value) => value,
                settings::PressureMode::Retain => 0.0,
            },
            _padding: [0; 3],
        }
    }

    fn diffusion_iterations(settings: &Settings) -> u32 {
        settings.diffusion_iterations.min(256)
    }
}

// Tiny startup/minimized domains do not warrant hundreds of solver passes.
// The full budget is available above an 80×80 logical-world-pixel area.
fn domain_iteration_cap(domain: grid::ScalingRatio, base: u32, cap: u32) -> u32 {
    ((cap as f32 * (domain.x() * domain.y() / 0.01).min(1.0)) as u32)
        .max(base.min(cap))
        .max(1)
}

fn pressure_iterations(
    settings: &Settings,
    domain: grid::ScalingRatio,
    size: wgpu::Extent3d,
) -> u32 {
    let simulation_domain = domain.simulation_domain();
    let inverse_cell = [
        size.width as f32 / (128.0 * simulation_domain[0]),
        size.height as f32 / (128.0 * simulation_domain[1]),
    ];
    // The reference square field used equal unit-cell metrics. Weighted
    // Jacobi propagation scales with sum(inverse_cell²) when refined.
    let world_refinement = ((inverse_cell[0].powi(2) + inverse_cell[1].powi(2)) / 2.0).sqrt();
    solver_iterations(
        settings.pressure_iterations,
        world_refinement,
        domain_iteration_cap(domain, settings.pressure_iterations, 512),
    )
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
/// them. The fluid rebuilds this as a unit when it changes size. Texture views
/// keep their textures alive, including during an in-flight resize.
struct Field {
    size: wgpu::Extent3d,

    velocity_texture_views: [wgpu::TextureView; 2],
    advection_forward_texture_view: wgpu::TextureView,
    divergence_texture_view: wgpu::TextureView,
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
                    | wgpu::TextureUsages::COPY_DST
                    | if cfg!(test) {
                        wgpu::TextureUsages::COPY_SRC
                    } else {
                        wgpu::TextureUsages::empty()
                    },
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

    scaling_ratio: grid::ScalingRatio,
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
    clear_pressure_pipeline: wgpu::ComputePipeline,
    subtract_gradient_pipeline: wgpu::ComputePipeline,

    resample_bind_group_layout: wgpu::BindGroupLayout,
    resample_pipeline: wgpu::ComputePipeline,

    last_pressure_index: Arc<Mutex<usize>>,
    last_velocity_index: Arc<Mutex<usize>>,
}

impl Context {
    pub fn update(&mut self, queue: &wgpu::Queue, settings: &Arc<Settings>) {
        // Update fluid settings needed on the CPU side
        let uniforms = FluidUniforms::new(settings, self.scaling_ratio, self.field.size);
        self.diffusion_iterations = FluidUniforms::diffusion_iterations(settings);
        self.pressure_mode = settings.pressure_mode;
        self.pressure_iterations =
            pressure_iterations(settings, self.scaling_ratio, self.field.size);

        // Update uniforms
        self.fluid_uniforms = uniforms;
        queue.write_buffer(
            &self.fluid_uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.fluid_uniforms]),
        );
    }

    /// Preserve velocity in the centered overlapping world domain when either
    /// the domain or texture quality changes. Newly revealed space starts at
    /// rest; pressure is rebuilt for the new cell metrics.
    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scaling_ratio: grid::ScalingRatio,
        settings: &Settings,
    ) {
        let size = scaling_ratio.texture_size(
            settings.fluid_size,
            grid::TextureBudget::for_base(
                settings.fluid_size,
                device.limits().max_texture_dimension_2d,
            ),
        );
        if size == self.field.size && scaling_ratio == self.scaling_ratio {
            return;
        }

        let field = Field::new(
            device,
            size,
            self.pressure_format,
            &self.layouts,
            &self.nearest_sampler,
        );

        let resample_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniform:resample_domain"),
            contents: bytemuck::cast_slice(&[
                scaling_ratio.x() / self.scaling_ratio.x(),
                scaling_ratio.y() / self.scaling_ratio.y(),
                0.0,
                0.0,
            ]),
            usage: wgpu::BufferUsages::UNIFORM,
        });
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
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: resample_uniform.as_entire_binding(),
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
        self.scaling_ratio = scaling_ratio;
        self.fluid_uniforms = FluidUniforms::new(settings, scaling_ratio, size);
        self.diffusion_iterations = FluidUniforms::diffusion_iterations(settings);
        self.pressure_iterations = pressure_iterations(settings, scaling_ratio, size);
        queue.write_buffer(
            &self.fluid_uniform_buffer,
            0,
            bytemuck::bytes_of(&self.fluid_uniforms),
        );
        *self.last_velocity_index.lock().unwrap() = 0;
        *self.last_pressure_index.lock().unwrap() = 0;
    }

    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        scaling_ratio: grid::ScalingRatio,
        _caps: BackendCaps,
        settings: &Arc<Settings>,
    ) -> Self {
        let size = scaling_ratio.texture_size(
            settings.fluid_size,
            grid::TextureBudget::for_base(
                settings.fluid_size,
                device.limits().max_texture_dimension_2d,
            ),
        );

        // Pressure uses integer stencil loads, so R32Float works even without
        // FLOAT32_FILTERABLE. Half precision stalls fine-grid Jacobi updates.
        let pressure_format = wgpu::TextureFormat::R32Float;

        // Uniforms

        let fluid_uniforms = FluidUniforms::new(settings, scaling_ratio, size);
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
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
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
                bind_group_layouts: &[
                    Some(&layouts.divergence),
                    Some(&layouts.velocity),
                    Some(&uniform_bind_group_layout),
                ],
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
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../shader/solve_pressure.comp.wgsl"
            ))),
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

        let clear_pressure_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader:clear_pressure"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../shader/clear_pressure.comp.wgsl"
            ))),
        });
        let clear_pressure_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipeline_layout:clear_pressure"),
                bind_group_layouts: &[Some(&uniform_bind_group_layout), Some(&layouts.pressure)],
                immediate_size: 0,
            });
        let clear_pressure_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("pipeline:clear_pressure"),
                layout: Some(&clear_pressure_layout),
                module: &clear_pressure_shader,
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
            diffusion_iterations: FluidUniforms::diffusion_iterations(settings),
            pressure_mode: settings.pressure_mode,
            pressure_iterations: pressure_iterations(settings, scaling_ratio, size),

            scaling_ratio,
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
            clear_pressure_pipeline,
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
        cpass.set_bind_group(2, &self.uniform_bind_group, &[]);
        cpass.set_bind_group(1, &self.field.velocity_bind_groups[*velocity_index], &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
    }

    pub fn solve_pressure<'cpass>(
        &'cpass self,
        _queue: &wgpu::Queue,
        cpass: &mut wgpu::ComputePass<'cpass>,
    ) {
        let mut pressure_index = self.last_pressure_index.lock().unwrap();
        let workgroup = self.field.workgroup_count();
        if matches!(self.pressure_mode, settings::PressureMode::ClearWith(_)) {
            // Encode the reset at this tick's position. Queue writes would all
            // run before the command buffer, so batched ticks would retain
            // the preceding tick's pressure despite ClearWith being selected.
            cpass.set_pipeline(&self.clear_pressure_pipeline);
            cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
            cpass.set_bind_group(
                1,
                &self.field.pressure_bind_groups[1 - *pressure_index],
                &[],
            );
            cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_fluid_retains_tuned_unit_cell_operators() {
        let settings = Settings::default();
        let domain = grid::ScalingRatio::new(1280, 800);
        let size = domain.texture_size(128, grid::TextureBudget::for_base(128, 8192));
        let uniforms = FluidUniforms::new(&settings, domain, size);
        let old_center_factor = 1.0 / (settings.viscosity * settings.fluid_timestep);
        let old_stencil_factor = 1.0 / (4.0 + old_center_factor);
        assert_eq!(uniforms.inverse_cell, [1.0; 2]);
        assert_eq!(uniforms.velocity_to_uv, [1.0 / 128.0; 2]);
        for weight in uniforms.diffusion_weight {
            assert!((weight - old_stencil_factor).abs() < 1e-7);
        }
        assert_eq!(
            pressure_iterations(&settings, domain, size),
            settings.pressure_iterations
        );
    }

    #[test]
    fn solver_work_grows_with_refinement_and_is_bounded() {
        assert_eq!(solver_iterations(19, 0.5, 512), 5);
        assert_eq!(solver_iterations(19, 1.0, 512), 19);
        assert_eq!(solver_iterations(19, 2.0, 512), 76);
        assert_eq!(solver_iterations(19, 4.0, 512), 304);
        assert_eq!(solver_iterations(u32::MAX, 100.0, 512), 512);
        assert_eq!(solver_iterations(0, 100.0, 512), 0);
    }

    #[test]
    fn world_stencils_and_tiny_domain_work_are_bounded() {
        let settings = Settings::default();
        let domain = grid::ScalingRatio::new(1280, 800);
        for base in [64, 128, 256, 512, 2048] {
            let size = domain.texture_size(base, grid::TextureBudget::for_base(base, 8192));
            let uniforms = FluidUniforms::new(&settings, domain, size);
            assert_eq!(
                FluidUniforms::diffusion_iterations(&settings),
                settings.diffusion_iterations
            );
            assert!(2.0 * uniforms.diffusion_weight.iter().sum::<f32>() <= 1.0);
            assert_eq!(uniforms.velocity_to_uv, [1.0 / 128.0; 2]);
        }
        let size = domain.texture_size(128, grid::TextureBudget::for_base(128, 8192));
        assert_eq!(
            pressure_iterations(&settings, domain.with_scale(2.0), size),
            76
        );
        assert_eq!(
            pressure_iterations(&settings, domain.with_scale(0.5), size),
            5
        );
        assert_eq!(domain.with_scale(0.5), grid::ScalingRatio::new(2560, 1600));
        let tiny = grid::ScalingRatio::new(1, 1);
        let size = tiny.texture_size(128, grid::TextureBudget::for_base(128, 8192));
        assert_eq!(
            pressure_iterations(&settings, tiny, size),
            settings.pressure_iterations
        );
    }

    fn gpu() -> (wgpu::Device, wgpu::Queue) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .expect("GPU tests require a compute-capable adapter");
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap()
    }

    fn context(device: &wgpu::Device, queue: &wgpu::Queue, base: u32) -> (Context, Arc<Settings>) {
        let settings = Arc::new(Settings {
            fluid_size: base,
            ..Settings::default()
        });
        let context = Context::new(
            device,
            queue,
            grid::ScalingRatio::new(1280, 800),
            BackendCaps {
                float32_filterable: false,
            },
            &settings,
        );
        (context, settings)
    }

    fn write_velocity(context: &Context, queue: &wgpu::Queue, f: impl Fn(f32, f32) -> [f32; 2]) {
        let size = context.field.size;
        let mut data = Vec::new();
        for y in 0..size.height {
            for x in 0..size.width {
                let v = f(
                    (x as f32 + 0.5) / size.width as f32,
                    (y as f32 + 0.5) / size.height as f32,
                );
                data.extend([
                    half::f16::from_f32(v[0]),
                    half::f16::from_f32(v[1]),
                    half::f16::ZERO,
                    half::f16::ZERO,
                ]);
            }
        }
        queue.write_texture(
            context
                .get_velocity_texture_view()
                .texture()
                .as_image_copy(),
            bytemuck::cast_slice(&data),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size.width * 8),
                rows_per_image: Some(size.height),
            },
            size,
        );
    }

    fn read_velocity(
        context: &Context,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Vec<[f32; 2]> {
        read_texture(context.get_velocity_texture_view(), device, queue)
    }

    fn read_texture(
        view: &wgpu::TextureView,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Vec<[f32; 2]> {
        let size = view.texture().size();
        let scalar = view.texture().format() == wgpu::TextureFormat::R32Float;
        let texel_bytes = if scalar { 4 } else { 8 };
        let stride = (size.width * texel_bytes).div_ceil(256) * 256;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("test:velocity_readback"),
            size: u64::from(stride) * u64::from(size.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            view.texture().as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(size.height),
                },
            },
            size,
        );
        queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| result.unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let mapped = slice.get_mapped_range().unwrap();
        mapped
            .chunks_exact(stride as usize)
            .flat_map(|row| {
                row[..size.width as usize * texel_bytes as usize]
                    .chunks_exact(texel_bytes as usize)
                    .map(|pixel| {
                        if scalar {
                            return [f32::from_le_bytes(pixel.try_into().unwrap()), 0.0];
                        }
                        [
                            half::f16::from_bits(u16::from_le_bytes([pixel[0], pixel[1]])).to_f32(),
                            half::f16::from_bits(u16::from_le_bytes([pixel[2], pixel[3]])).to_f32(),
                        ]
                    })
            })
            .collect()
    }

    #[test]
    #[ignore = "requires a compute-capable GPU; run with --ignored"]
    fn gpu_resize_preserves_centered_world_overlap_and_velocity_units() {
        let (device, queue) = gpu();
        let (mut context, settings) = context(&device, &queue, 128);
        // A ramp detects spatial stretching; the nonzero constant detects
        // amplitude scaling. The new side strips must be at rest.
        write_velocity(&context, &queue, |u, _| [2.0 + u, -3.0]);
        context.resize(
            &device,
            &queue,
            grid::ScalingRatio::new(2560, 800),
            &settings,
        );
        let values = read_velocity(&context, &device, &queue);
        let size = context.field.size;
        for x in 0..size.width {
            let uv = (x as f32 + 0.5) / size.width as f32;
            let old_uv = (uv - 0.5) * 2.0 + 0.5;
            let value = values[(size.height / 2 * size.width + x) as usize];
            if !(0.0..=1.0).contains(&old_uv) {
                assert_eq!(value, [0.0, 0.0]);
            } else {
                assert!((value[0] - (2.0 + old_uv)).abs() < 0.004);
                assert_eq!(value[1], -3.0);
            }
        }
        let higher = Settings {
            fluid_size: 256,
            ..(*settings).clone()
        };
        context.resize(&device, &queue, grid::ScalingRatio::new(2560, 800), &higher);
        let values = read_velocity(&context, &device, &queue);
        let size = context.field.size;
        let middle = values[(size.height / 2 * size.width + size.width / 2) as usize];
        assert!((middle[0] - 2.5).abs() < 0.01);
        assert_eq!(middle[1], -3.0);

        // Same aspect and texel count, but a different logical world extent.
        // This used to skip resampling entirely.
        context.resize(
            &device,
            &queue,
            grid::ScalingRatio::new(5120, 1600),
            &higher,
        );
        let values = read_velocity(&context, &device, &queue);
        let middle_row = size.height / 2 * size.width;
        assert_eq!(values[(middle_row + size.width / 4) as usize], [0.0, 0.0]);
        assert!((values[(middle_row + size.width / 2) as usize][0] - 2.5).abs() < 0.02);
    }

    #[test]
    #[ignore = "requires a compute-capable GPU; run with --ignored"]
    fn gpu_advection_uses_world_distance_at_every_quality() {
        let (device, queue) = gpu();
        for base in [128, 256] {
            let (mut context, settings) = context(&device, &queue, base);
            let settings = Arc::new(Settings {
                fluid_timestep: 1.0,
                ..(*settings).clone()
            });
            context.update(&queue, &settings);
            write_velocity(&context, &queue, |u, _| {
                [8.0, (4.0 * std::f32::consts::TAU * u).sin()]
            });
            let mut encoder = device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_compute_pass(&Default::default());
                context.advect_forward(&queue, &mut pass);
            }
            queue.submit([encoder.finish()]);
            let values = read_texture(
                context.get_advection_forward_texture_view(),
                &device,
                &queue,
            );
            let size = context.field.size;
            for x in size.width / 4..3 * size.width / 4 {
                let u = (x as f32 + 0.5) / size.width as f32;
                let expected = (4.0 * std::f32::consts::TAU * (u - 8.0 / 128.0)).sin();
                let value = values[(size.height / 2 * size.width + x) as usize];
                assert_eq!(value[0], 8.0);
                assert!(
                    (value[1] - expected).abs() < 0.006,
                    "advection drift at quality{base}: {value:?} vs{expected}"
                );
            }
        }
    }

    #[test]
    #[ignore = "requires a compute-capable GPU; run with --ignored"]
    fn gpu_pressure_reset_is_ordered_between_batched_ticks() {
        let (device, queue) = gpu();
        for mode in [
            settings::PressureMode::ClearWith(0.25),
            settings::PressureMode::Retain,
        ] {
            let mut outputs = Vec::new();
            for batched in [false, true] {
                let settings = Arc::new(Settings {
                    pressure_mode: mode,
                    ..Settings::default()
                });
                let context = Context::new(
                    &device,
                    &queue,
                    grid::ScalingRatio::new(1280, 800),
                    BackendCaps {
                        float32_filterable: false,
                    },
                    &settings,
                );
                write_velocity(&context, &queue, |u, v| {
                    [
                        (4.0 * std::f32::consts::TAU * u).sin(),
                        (2.0 * std::f32::consts::TAU * v).sin(),
                    ]
                });
                for _ in 0..if batched { 1 } else { 6 } {
                    let mut encoder = device.create_command_encoder(&Default::default());
                    {
                        let mut pass = encoder.begin_compute_pass(&Default::default());
                        for _ in 0..if batched { 6 } else { 1 } {
                            context.calculate_divergence(&mut pass);
                            context.solve_pressure(&queue, &mut pass);
                            context.subtract_gradient(&mut pass);
                        }
                    }
                    queue.submit([encoder.finish()]);
                }
                outputs.push(read_velocity(&context, &device, &queue));
            }
            assert_eq!(
                outputs[0], outputs[1],
                "batched ticks changed pressure reset semantics for{mode:?}"
            );
        }
        // ClearWith also applies when no Jacobi iterations are requested;
        // Retain must then leave that nonzero value untouched.
        let settings = Arc::new(Settings {
            pressure_mode: settings::PressureMode::ClearWith(0.25),
            pressure_iterations: 0,
            ..Settings::default()
        });
        let mut context = Context::new(
            &device,
            &queue,
            grid::ScalingRatio::new(1280, 800),
            BackendCaps {
                float32_filterable: false,
            },
            &settings,
        );
        for retain in [false, true] {
            if retain {
                context.update(
                    &queue,
                    &Arc::new(Settings {
                        pressure_mode: settings::PressureMode::Retain,
                        ..(*settings).clone()
                    }),
                );
            }
            let mut encoder = device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_compute_pass(&Default::default());
                context.solve_pressure(&queue, &mut pass);
            }
            queue.submit([encoder.finish()]);
            let values = read_texture(context.get_pressure_texture_view(), &device, &queue);
            assert!(values.iter().all(|value| value[0] == 0.25));
        }
    }

    fn mode_amplitude(values: &[[f32; 2]], size: wgpu::Extent3d, axis: usize) -> f32 {
        let mut numerator = 0.0;
        let mut denominator = 0.0;
        // Stay clear of boundary cells to measure interior solver response.
        for y in size.height / 4..3 * size.height / 4 {
            for x in size.width / 4..3 * size.width / 4 {
                let position = if axis == 0 {
                    (x as f32 + 0.5) / size.width as f32
                } else {
                    (y as f32 + 0.5) / size.height as f32
                };
                let mode = (4.0 * std::f32::consts::TAU * position).sin();
                numerator += values[(y * size.width + x) as usize][axis] * mode;
                denominator += mode * mode;
            }
        }
        numerator / denominator
    }

    #[test]
    #[ignore = "requires a compute-capable GPU; run with --ignored"]
    fn gpu_pressure_reach_tracks_fixed_world_wavelength_when_domain_grows() {
        let (device, queue) = gpu();
        let settings = Arc::new(Settings::default());
        for axis in [0, 1] {
            let mut amplitudes = Vec::new();
            for domain in [
                grid::ScalingRatio::new(1280, 800),
                grid::ScalingRatio::new(2560, 1600),
            ] {
                let extent = domain.simulation_domain()[axis];
                let context = Context::new(
                    &device,
                    &queue,
                    domain,
                    BackendCaps {
                        float32_filterable: false,
                    },
                    &settings,
                );
                // The second domain contains eight cycles of the same wave;
                // its larger cells must not increase pressure's world reach.
                let mode =
                    |uv: f32| (4.0 * std::f32::consts::TAU * ((uv - 0.5) * extent + 0.5)).sin();
                write_velocity(&context, &queue, |u, v| {
                    let mut value = [0.0; 2];
                    value[axis] = mode(if axis == 0 { u } else { v });
                    value
                });
                let mut encoder = device.create_command_encoder(&Default::default());
                {
                    let mut pass = encoder.begin_compute_pass(&Default::default());
                    context.calculate_divergence(&mut pass);
                    context.solve_pressure(&queue, &mut pass);
                    context.subtract_gradient(&mut pass);
                }
                queue.submit([encoder.finish()]);
                let values = read_velocity(&context, &device, &queue);
                let size = context.field.size;
                let mut numerator = 0.0;
                let mut denominator = 0.0;
                for y in size.height / 4..3 * size.height / 4 {
                    for x in size.width / 4..3 * size.width / 4 {
                        let uv = if axis == 0 {
                            (x as f32 + 0.5) / size.width as f32
                        } else {
                            (y as f32 + 0.5) / size.height as f32
                        };
                        // Measure the same central world interval in each domain.
                        if ((uv - 0.5) * extent).abs() > 0.25 {
                            continue;
                        }
                        numerator += values[(y * size.width + x) as usize][axis] * mode(uv);
                        denominator += mode(uv).powi(2);
                    }
                }
                amplitudes.push(numerator / denominator);
            }
            eprintln!("fixed-world pressure, axis={axis}: reference/double domain amplitudes={amplitudes:?}");
            assert!(amplitudes.iter().all(|value| *value > 0.8 && *value < 0.85));
            assert!((amplitudes[0] - amplitudes[1]).abs() < 0.01,
                "a larger logical domain changed the pressure response of the same world wavelength");
        }
    }

    #[test]
    #[ignore = "requires a compute-capable GPU; run with --ignored"]
    fn gpu_broad_pressure_and_diffusion_response_survives_quality_changes() {
        let (device, queue) = gpu();
        for domain in [
            grid::ScalingRatio::new(1280, 800),
            grid::ScalingRatio::new(800, 1280),
        ] {
            for axis in [0, 1] {
                for pressure in [true, false] {
                    let mut amplitudes = Vec::new();
                    for base in [64, 128, 256] {
                        let settings = Arc::new(Settings {
                            fluid_size: base,
                            diffusion_iterations: 24,
                            ..Settings::default()
                        });
                        let context = Context::new(
                            &device,
                            &queue,
                            domain,
                            BackendCaps {
                                float32_filterable: false,
                            },
                            &settings,
                        );
                        write_velocity(&context, &queue, |u, v| {
                            let mut value = [0.0; 2];
                            value[axis] =
                                (4.0 * std::f32::consts::TAU * if axis == 0 { u } else { v }).sin();
                            value
                        });
                        let mut encoder = device.create_command_encoder(&Default::default());
                        {
                            let mut pass = encoder.begin_compute_pass(&Default::default());
                            if pressure {
                                context.calculate_divergence(&mut pass);
                                context.solve_pressure(&queue, &mut pass);
                                context.subtract_gradient(&mut pass);
                            } else {
                                context.diffuse(&mut pass);
                            }
                        }
                        queue.submit([encoder.finish()]);
                        let values = read_velocity(&context, &device, &queue);
                        assert!(values.iter().flatten().all(|v| v.is_finite()));
                        amplitudes.push(mode_amplitude(&values, context.field.size, axis));
                    }
                    eprintln!("domain={domain:?} axis={axis} pressure={pressure}: amplitude at64/128/256 = {amplitudes:?}");
                    assert!(
                        amplitudes.iter().all(|a| *a > 0.3 && *a < 0.99),
                        "nontrivial positive solver response required: {amplitudes:?}"
                    );
                    assert!(
                        (amplitudes[1] - amplitudes[2]).abs() < 0.004,
                        "world-space response changed with resolution: {amplitudes:?}"
                    );
                    assert!(
                        (amplitudes[0] - amplitudes[1]).abs() < 0.025,
                        "coarse-grid response changed excessively: {amplitudes:?}"
                    );
                }
            }
        }
    }
}
