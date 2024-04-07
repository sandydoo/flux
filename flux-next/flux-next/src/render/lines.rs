use crate::grid::Grid;
use crate::settings::{ColorMode, Settings};

use bytemuck::Zeroable;
use std::borrow::Cow;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct LineUniforms {
    aspect: f32,                  // 0
    zoom: f32,                    // 4
    line_width: f32,              // 8
    line_length: f32,             // 12
    line_begin_offset: f32,       // 16
    line_variance: f32,           // 20
    line_noise_scale: [f32; 2],   // 24
    line_noise_offset_1: f32,     // 32
    line_noise_offset_2: f32,     // 36
    line_noise_blend_factor: f32, // 40

    // 0 => The "Original" color preset
    // 1 => A color preset with a color wheel
    // 2 => Sample colors from a texture
    // 3 => Sample colors from a texture with SRGB (unsupported)
    color_mode: u32, // 44

    delta_time: f32, // 48
    _padding: [u32; 3],
}

impl LineUniforms {
    fn new(screen_size: wgpu::Extent3d, grid: &Grid, settings: &Settings) -> Self {
        // TODO: can we compute the scale factor from the grid?
        let line_scale_factor =
            get_line_scale_factor(screen_size.width as f32, screen_size.height as f32);

        Self {
            aspect: grid.aspect_ratio,
            zoom: settings.view_scale,
            line_width: settings.view_scale * settings.line_width * line_scale_factor,
            line_length: settings.view_scale * settings.line_length * line_scale_factor,
            line_begin_offset: settings.line_begin_offset,
            line_variance: settings.line_variance,
            line_noise_scale: [64.0 * grid.scaling_ratio.x(), 64.0 * grid.scaling_ratio.y()],
            line_noise_offset_1: 0.0,
            line_noise_offset_2: 0.0,
            line_noise_blend_factor: 0.0,
            color_mode: settings.color_mode.clone().into(),
            delta_time: 1.0 / 60.0, // Initial value, will be updated every frame
            _padding: [0; 3],
        }
    }

    fn tick(&mut self, timestep: f32, elapsed_time: f32) -> &mut Self {
        const BLEND_THRESHOLD: f32 = 4.0;
        const BASE_OFFSET: f32 = 0.0015;

        let perturb = 1.0 + 0.2 * (0.010 * elapsed_time * std::f32::consts::TAU).sin();
        let offset = BASE_OFFSET * perturb;
        self.line_noise_offset_1 += offset;

        if self.line_noise_offset_1 > BLEND_THRESHOLD {
            self.line_noise_offset_2 += offset;
            self.line_noise_blend_factor += BASE_OFFSET;
        }

        if self.line_noise_blend_factor > 1.0 {
            self.line_noise_offset_1 = self.line_noise_offset_2;
            self.line_noise_offset_2 = 0.0;
            self.line_noise_blend_factor = 0.0;
        }

        self.delta_time = timestep;

        self
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Line {
    endpoint: [f32; 2],
    velocity: [f32; 2],
    color: [f32; 4],
    color_velocity: [f32; 3],
    width: f32,
}

pub struct Context {
    line_count: u32,
    work_group_count: u32,
    frame_num: usize,

    line_vertex_buffer: wgpu::Buffer,
    endpoint_vertex_buffer: wgpu::Buffer,
    basepoints_buffer: wgpu::Buffer,
    line_uniforms: LineUniforms,
    line_uniform_buffer: wgpu::Buffer,
    line_buffers: Vec<wgpu::Buffer>,

    linear_sampler: wgpu::Sampler,
    uniform_bind_group_layout: wgpu::BindGroupLayout,
    uniform_bind_group: wgpu::BindGroup,
    lines_bind_group_layout: wgpu::BindGroupLayout,
    line_bind_groups: Vec<wgpu::BindGroup>,

    pub color_mode: u32,
    color_texture_sampler: wgpu::Sampler,
    color_texture_view: wgpu::TextureView,
    color_buffer: wgpu::Buffer,
    color_bind_group_layout: wgpu::BindGroupLayout,
    color_bind_group: wgpu::BindGroup,

    place_lines_pipeline: wgpu::ComputePipeline,
    draw_line_pipeline: wgpu::RenderPipeline,
    draw_endpoint_pipeline: wgpu::RenderPipeline,
}

impl Context {
    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_size: wgpu::Extent3d,
        grid: &Grid,
        settings: &Settings,
    ) {
        self.line_uniforms = {
            let mut new_line_uniforms = LineUniforms::new(screen_size, grid, settings);
            new_line_uniforms.line_noise_offset_1 = self.line_uniforms.line_noise_offset_1;
            new_line_uniforms.line_noise_offset_2 = self.line_uniforms.line_noise_offset_2;
            new_line_uniforms.line_noise_blend_factor = self.line_uniforms.line_noise_blend_factor;

            new_line_uniforms
        };

        if let ColorMode::Preset(preset) = settings.color_mode {
            if let Some(color_wheel) = preset.to_color_wheel() {
                self.color_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("buffer:color"),
                    size: 4 * (color_wheel.len() as u64),
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
                    mapped_at_creation: false,
                });

                queue.write_buffer(&self.color_buffer, 0, &bytemuck::cast_slice(&[color_wheel]));

                self.color_mode = 1;
                self.update_color_bindings(device, queue, None, None);
            }
        }

        queue.write_buffer(
            &self.line_uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.line_uniforms]),
        );
    }

    pub fn update_color_bindings(
        self: &mut Self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        some_color_texture_view: Option<wgpu::TextureView>,
        some_color_buffer: Option<wgpu::Buffer>,
    ) {
        if let Some(color_texture_view) = some_color_texture_view {
            self.color_texture_view = color_texture_view;
            self.color_mode = 2;
        }
        if let Some(color_buffer) = some_color_buffer {
            self.color_buffer = color_buffer;
            self.color_mode = 1;
        }

        self.color_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group:color"),
            layout: &self.color_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.color_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.color_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        self.update_line_color_mode(device, queue);
    }

    pub fn tick_line_uniforms(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        timestep: f32,
        elapsed_time: f32,
    ) {
        self.line_uniforms.tick(timestep, elapsed_time);

        queue.write_buffer(
            &self.line_uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.line_uniforms]),
        );
    }

    pub fn update_line_color_mode(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.line_uniforms.color_mode = self.color_mode;

        queue.write_buffer(
            &self.line_uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.line_uniforms]),
        );
    }

    // TODO: resample the line state in a compute shader
    // TODO: dedupe with new
    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_size: wgpu::Extent3d,
        grid: &Grid,
        settings: &Settings,
    ) {
        self.update(device, queue, screen_size, grid, settings);

        let basepoints_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("buffer:basepoints"),
            contents: bytemuck::cast_slice(&grid.basepoints),
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST,
        });

        let lines = vec![Line::zeroed(); grid.line_count as usize];

        let line_buffers = (0..2)
            .map(|i| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(format!("buffer:lines_{}", i).as_str()),
                    contents: bytemuck::cast_slice(&lines),
                    usage: wgpu::BufferUsages::VERTEX
                        | wgpu::BufferUsages::STORAGE
                        | wgpu::BufferUsages::COPY_DST,
                })
            })
            .collect::<Vec<_>>();

        let line_bind_groups = (0..2)
            .map(|i| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("bind_group:lines"),
                    layout: &self.lines_bind_group_layout,
                    entries: &[
                        // lines
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: line_buffers[i].as_entire_binding(),
                        },
                        // out_lines
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: line_buffers[(i + 1) % 2].as_entire_binding(),
                        },
                    ],
                })
            })
            .collect::<Vec<_>>();

        self.uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group:uniforms"),
            layout: &self.uniform_bind_group_layout,
            entries: &[
                // uniforms
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.line_uniform_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                // basepoints
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: basepoints_buffer.as_entire_binding(),
                },
                // linear_sampler
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                },
                // color_texture_sampler
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.color_texture_sampler),
                },
            ],
        });

        self.line_count = grid.line_count;
        self.work_group_count = ((grid.line_count as f32) / 64.0).ceil() as u32;
        self.line_buffers = line_buffers;
        self.line_bind_groups = line_bind_groups;
        self.basepoints_buffer = basepoints_buffer;
    }

    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        swapchain_format: wgpu::TextureFormat,
        screen_size: wgpu::Extent3d,
        grid: &Grid,
        settings: &Settings,
    ) -> Self {
        let line_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("buffer:vertices"),
            contents: bytemuck::cast_slice(&LINE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let endpoint_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("buffer:endpoints"),
            contents: bytemuck::cast_slice(&[ENDPOINT_VERTICES]),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let line_uniforms = LineUniforms::new(screen_size, grid, settings);

        let line_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("buffer:LineUniforms"),
            contents: bytemuck::cast_slice(&[line_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let basepoints_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("buffer:basepoints"),
            contents: bytemuck::cast_slice(&grid.basepoints),
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST,
        });

        let lines = vec![Line::zeroed(); grid.line_count as usize];

        let line_buffers = (0..2)
            .map(|i| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(format!("buffer:lines_{}", i).as_str()),
                    contents: bytemuck::cast_slice(&lines),
                    usage: wgpu::BufferUsages::VERTEX
                        | wgpu::BufferUsages::STORAGE
                        | wgpu::BufferUsages::COPY_DST,
                })
            })
            .collect::<Vec<_>>();

        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sampler:linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let color_texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sampler:color_texture"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::MirrorRepeat,
            address_mode_v: wgpu::AddressMode::MirrorRepeat,
            ..Default::default()
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bind_group_layout:uniforms"),
                entries: &[
                    // uniforms
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE
                            | wgpu::ShaderStages::VERTEX
                            | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // basepoints
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // linear_sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // color_texture_sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group:uniforms"),
            layout: &uniform_bind_group_layout,
            entries: &[
                // uniforms
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &line_uniform_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                // basepoints
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: basepoints_buffer.as_entire_binding(),
                },
                // linear_sampler
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&linear_sampler),
                },
                // color_texture_sampler
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&color_texture_sampler),
                },
            ],
        });

        let lines_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bind_group_layout:lines"),
                entries: &[
                    // lines
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // out_lines
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let line_bind_groups = (0..2)
            .map(|i| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("bind_group:lines"),
                    layout: &lines_bind_group_layout,
                    entries: &[
                        // lines
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: line_buffers[i].as_entire_binding(),
                        },
                        // out_lines
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: line_buffers[(i + 1) % 2].as_entire_binding(),
                        },
                    ],
                })
            })
            .collect::<Vec<_>>();

        let color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 100,
                height: 100,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            view_formats: &[],
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        });

        let color_texture_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let color_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("buffer:color"),
            size: 4 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let color_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
        let color_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &color_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&color_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &color_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        // TODO: reuse layout from fluid
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

        let place_lines_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipeline_layout:place_lines"),
                bind_group_layouts: &[
                    &uniform_bind_group_layout,
                    &lines_bind_group_layout,
                    &color_bind_group_layout,
                    &velocity_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let place_lines_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader:place_lines"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../shader/place_lines.comp.wgsl"
            ))),
        });

        let place_lines_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("pipeline:place_lines"),
                layout: Some(&place_lines_pipeline_layout),
                module: &place_lines_shader,
                entry_point: "main",
            });

        let draw_line_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipeline_layout:draw_line"),
                bind_group_layouts: &[&uniform_bind_group_layout],
                push_constant_ranges: &[],
            });

        let draw_line_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader:draw_line"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../../shader/line.wgsl"))),
        });

        let vertex_buffer_layouts = [
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Line>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![
                            0 => Float32x2, 1 => Float32x2, 2 => Float32x4, 3 => Float32x3, 4 => Float32],
            },
            wgpu::VertexBufferLayout {
                array_stride: 2 * std::mem::size_of::<f32>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![5 => Float32x2],
            },
            wgpu::VertexBufferLayout {
                array_stride: 2 * std::mem::size_of::<f32>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![6 => Float32x2],
            },
        ];

        let color_targets = [Some(wgpu::ColorTargetState {
            format: swapchain_format,
            blend: Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            write_mask: wgpu::ColorWrites::ALL,
        })];

        let draw_line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pipeline:draw_line"),
            layout: Some(&draw_line_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &draw_line_shader,
                entry_point: "main_vs",
                buffers: &vertex_buffer_layouts,
            },
            fragment: Some(wgpu::FragmentState {
                module: &draw_line_shader,
                entry_point: "main_fs",
                targets: &color_targets,
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let draw_endpoint_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipeline_layout:draw_endpoint"),
                bind_group_layouts: &[&uniform_bind_group_layout],
                push_constant_ranges: &[],
            });

        let draw_endpoint_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader:draw_endpoint"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../shader/endpoint.wgsl"
            ))),
        });

        // TODO: reuse draw_line layout
        let draw_endpoint_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("pipeline:draw_endpoint"),
                layout: Some(&draw_endpoint_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &draw_endpoint_shader,
                    entry_point: "main_vs",
                    buffers: &vertex_buffer_layouts,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &draw_endpoint_shader,
                    entry_point: "main_fs",
                    targets: &color_targets,
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        let work_group_count = ((grid.line_count as f32) / 64.0).ceil() as u32;

        Self {
            line_count: grid.line_count,
            work_group_count,
            frame_num: 0,

            line_vertex_buffer,
            endpoint_vertex_buffer,
            basepoints_buffer,
            line_uniforms,
            line_uniform_buffer,
            line_buffers,

            linear_sampler,
            color_texture_sampler,
            uniform_bind_group_layout,
            uniform_bind_group,
            lines_bind_group_layout,
            line_bind_groups,

            color_mode: line_uniforms.color_mode,
            color_texture_view,
            color_buffer,
            color_bind_group_layout,
            color_bind_group,

            place_lines_pipeline,
            draw_line_pipeline,
            draw_endpoint_pipeline,
        }
    }

    pub fn place_lines<'cpass>(
        &'cpass mut self,
        cpass: &mut wgpu::ComputePass<'cpass>,
        velocity_bind_group: &'cpass wgpu::BindGroup,
    ) {
        cpass.set_pipeline(&self.place_lines_pipeline);
        cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        cpass.set_bind_group(1, &self.line_bind_groups[self.frame_num], &[]);
        cpass.set_bind_group(2, &self.color_bind_group, &[]);
        cpass.set_bind_group(3, velocity_bind_group, &[]);
        cpass.dispatch_workgroups(self.work_group_count, 1, 1);

        self.frame_num = 1 - self.frame_num;
    }

    pub fn draw_lines<'rpass>(&'rpass self, rpass: &mut wgpu::RenderPass<'rpass>) {
        rpass.set_pipeline(&self.draw_line_pipeline);
        rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        rpass.set_vertex_buffer(0, self.line_buffers[self.frame_num].slice(..));
        rpass.set_vertex_buffer(1, self.basepoints_buffer.slice(..));
        rpass.set_vertex_buffer(2, self.line_vertex_buffer.slice(..));
        rpass.draw(0..6, 0..self.line_count);
    }

    pub fn draw_endpoints<'rpass>(&'rpass self, rpass: &mut wgpu::RenderPass<'rpass>) {
        rpass.set_pipeline(&self.draw_endpoint_pipeline);
        rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        rpass.set_vertex_buffer(0, self.line_buffers[self.frame_num].slice(..));
        rpass.set_vertex_buffer(1, self.basepoints_buffer.slice(..));
        rpass.set_vertex_buffer(2, self.endpoint_vertex_buffer.slice(..));
        rpass.draw(0..6, 0..self.line_count);
    }
}

fn get_line_scale_factor(width: f32, height: f32) -> f32 {
    let aspect_ratio = width / height;
    let p = 1.0 / aspect_ratio;
    1.0 / ((1.0 - p) * width + p * height).min(2000.0)
}

#[rustfmt::skip]
pub static LINE_VERTICES: [f32; 12] = [
    -0.5, 0.0,
    -0.5, 1.0,
     0.5, 1.0,
    -0.5, 0.0,
     0.5, 1.0,
     0.5, 0.0,
];

#[rustfmt::skip]
pub static ENDPOINT_VERTICES: [f32; 12] = [
    -1.0, -1.0,
    -1.0,  1.0,
     1.0, -1.0,
     1.0, -1.0,
    -1.0,  1.0,
     1.0,  1.0,
];
