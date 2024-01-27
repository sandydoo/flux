use crate::settings::Settings;

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
    advection_bind_group: wgpu::BindGroup,

    clear_pressure_pipeline: wgpu::ComputePipeline,
    advection_pipeline: wgpu::ComputePipeline,
    // adjust_advection_pipeline: wgpu::ComputePipeline,
    diffusion_pipeline: wgpu::ComputePipeline,
    // divergence_pipeline: wgpu::ComputePipeline,
    // pressure_pipeline: wgpu::ComputePipeline,
    // subtract_gradient_pipeline: wgpu::ComputePipeline,
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
            timestep: 1.0 / settings.fluid_timestep,
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
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("texture:velocity_2"),
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

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &advection_forward_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            // TODO: remove debugging values
            bytemuck::cast_slice(&vec![0.5f32; (2 * width * height) as usize]),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(2 * 4 * width),
                rows_per_image: Some(height),
            },
            size,
        );

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
                label: Some("texture:pressure_1"),
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
                label: Some("texture:pressure_2"),
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
                label: Some("view:velocity_1"),
                ..Default::default()
            }),
            velocity_textures[1].create_view(&wgpu::TextureViewDescriptor {
                label: Some("view:velocity_2"),
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

        let advection_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Advection bind group layout"),
                entries: &[
                    // FluidUniforms
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
                    // velocityTexture
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
                    // velocitySampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // outTexture
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
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

        let advection_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Advection bind group"),
            layout: &advection_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &fluid_uniform_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                // TODO: needs to switch between velocity textures
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&velocity_texture_views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&linear_sampler),
                },
                // TODO: this needs to handle both reverse and forward
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&advection_forward_texture_view),
                },
            ],
        });

        let advection_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Advection layout"),
                bind_group_layouts: &[&advection_bind_group_layout],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::COMPUTE,
                    range: 0..8,
                }],
            });

        let advection_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Advection shader"),
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

        let diffusion_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Diffusion"),
            layout: Some(&advection_pipeline_layout),
            module: &advection_shader,
            entry_point: "main",
        });

        Self {
            fluid_size: [width as f32, height as f32],
            fluid_size_3d: size,
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
            advection_bind_group,

            clear_pressure_pipeline,
            advection_pipeline,
            diffusion_pipeline,
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
        cpass.set_bind_group(0, &self.advection_bind_group, &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
    }

    pub fn advect_reverse<'cpass>(&'cpass self, cpass: &mut wgpu::ComputePass<'cpass>) {
        let workgroup = self.get_workgroup_size();
        cpass.set_pipeline(&self.advection_pipeline);
        cpass.set_push_constants(0, bytemuck::cast_slice(&[-1.0]));
        cpass.set_bind_group(0, &self.advection_bind_group, &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
    }

    pub fn adjust_advection(&self) {}

    pub fn diffuse<'cpass>(&'cpass self, cpass: &mut wgpu::ComputePass<'cpass>) {
        let workgroup = self.get_workgroup_size();
        cpass.set_pipeline(&self.diffusion_pipeline);
        cpass.set_bind_group(0, &self.advection_bind_group, &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
    }

    pub fn calculate_divergence(&self) {}

    pub fn clear_pressure(&self) {}

    pub fn solve_pressure(&self) {}

    pub fn subtract_gradient(&self) {}

    pub fn get_fluid_size(&self) -> wgpu::Extent3d {
        self.fluid_size_3d
    }

    // TODO: fix texture
    pub fn get_velocity_texture_view(&self) -> &wgpu::TextureView {
        &self.velocity_texture_views[1]
    }

    pub fn get_advection_forward_texture_view(&self) -> &wgpu::TextureView {
        &self.advection_forward_texture_view
    }

    pub fn get_velocity_bind_group(&self) -> &wgpu::BindGroup {
        // TODO: fix indexing
        &self.velocity_bind_groups[0]
    }
}
