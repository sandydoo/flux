use crate::grid::Grid;
use crate::settings::{self, Settings};

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
    padding: f32,
}

impl LineUniforms {
    fn tick(&mut self, elapsed_time: f32) -> &mut Self {
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

    vertices_buffer: wgpu::Buffer,
    basepoints_buffer: wgpu::Buffer,
    line_uniforms: LineUniforms,
    line_uniform_buffer: wgpu::Buffer,
    line_buffers: Vec<wgpu::Buffer>,

    uniform_bind_group: wgpu::BindGroup,
    line_bind_groups: Vec<wgpu::BindGroup>,

    place_lines_pipeline: wgpu::ComputePipeline,
    draw_line_pipeline: wgpu::RenderPipeline,
    draw_endpoint_pipeline: wgpu::RenderPipeline,
}

impl Context {
    pub fn update_line_uniforms(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        elapsed_time: f32,
    ) {
        self.line_uniforms.tick(elapsed_time);

        queue.write_buffer(
            &self.line_uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.line_uniforms]),
        );
    }

    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        swapchain_format: wgpu::TextureFormat,
        screen_size: wgpu::Extent3d,
        grid: &Grid,
        settings: &Settings,
        velocity_texture_view: &wgpu::TextureView,
    ) -> Self {
        log::info!("LINE COUNT: {:?}", grid.line_count);
        let vertices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("buffer:vertices"),
            contents: bytemuck::cast_slice(&LINE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let line_scale_factor =
            get_line_scale_factor(screen_size.width as f32, screen_size.height as f32);

        let line_uniforms = LineUniforms {
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
            delta_time: 1.0 / 120.0, // TODO: fix fps
            padding: 0.0,
        };

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

        let mut lines = vec![Line::zeroed(); grid.line_count as usize];

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
            ..Default::default()
        });

        let color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("texture:color"),
            // TODO: should this always be the same size? or should we upload a new one each time?
            size: wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            view_formats: &[],
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        });

        let color_texture_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());

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
                    // color_texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    // velocity_texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
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
                // color_texture
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&color_texture_view),
                },
                // velocity_texture
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&velocity_texture_view),
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

        let place_lines_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipeline_layout:place_lines"),
                bind_group_layouts: &[&uniform_bind_group_layout, &lines_bind_group_layout],
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
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![5 => Float32x2],
            },
            wgpu::VertexBufferLayout {
                array_stride: 2 * std::mem::size_of::<f32>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
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

            vertices_buffer,
            basepoints_buffer,
            line_uniforms,
            line_uniform_buffer,
            line_buffers,

            uniform_bind_group,
            line_bind_groups,

            place_lines_pipeline,
            draw_line_pipeline,
            draw_endpoint_pipeline,
        }
    }

    pub fn place_lines<'cpass>(&'cpass mut self, cpass: &mut wgpu::ComputePass<'cpass>) {
        cpass.set_pipeline(&self.place_lines_pipeline);
        cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        cpass.set_bind_group(1, &self.line_bind_groups[self.frame_num], &[]);
        cpass.dispatch_workgroups(self.work_group_count, 1, 1);

        self.frame_num = 1 - self.frame_num;
    }

    pub fn draw_lines<'rpass>(&'rpass self, rpass: &mut wgpu::RenderPass<'rpass>) {
        rpass.set_pipeline(&self.draw_line_pipeline);
        rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        rpass.set_vertex_buffer(0, self.line_buffers[self.frame_num].slice(..));
        rpass.set_vertex_buffer(1, self.vertices_buffer.slice(..));
        rpass.set_vertex_buffer(2, self.basepoints_buffer.slice(..));
        rpass.draw(0..6, 0..self.line_count);
    }

    pub fn draw_endpoints<'rpass>(&'rpass self, rpass: &mut wgpu::RenderPass<'rpass>) {
        rpass.set_pipeline(&self.draw_endpoint_pipeline);
        rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
        rpass.set_vertex_buffer(0, self.line_buffers[self.frame_num].slice(..));
        rpass.set_vertex_buffer(1, self.vertices_buffer.slice(..));
        rpass.set_vertex_buffer(2, self.basepoints_buffer.slice(..));
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
