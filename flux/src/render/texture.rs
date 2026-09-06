use std::borrow::Cow;
use wgpu::util::DeviceExt;

pub struct Context {
    _bind_group_layout: wgpu::BindGroupLayout,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    view_uniform_buffer: wgpu::Buffer,
    texture_bind_groups: Vec<(String, wgpu::BindGroup)>,
    _sampler: wgpu::Sampler,
    _pipeline_layout: wgpu::PipelineLayout,
    pipeline: wgpu::RenderPipeline,
    scalar_pipeline: wgpu::RenderPipeline,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
}

impl Context {
    pub fn new(
        device: &wgpu::Device,
        swapchain_format: wgpu::TextureFormat,
        texture_views: &[(&str, &wgpu::TextureView)],
    ) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    // The sampler below uses Nearest filtering, so this binding
                    // is non-filtering. Declaring it as `Filtering` would
                    // gratuitously require the bound texture format to support
                    // linear filtering — breaking the divergence (R32Float)
                    // view in the FLOAT32_FILTERABLE fallback path.
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        // Pairs with the NonFiltering sampler above; the debug
                        // viz only does nearest sampling so we don't need the
                        // texture format to be filterable.
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                }],
            });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let quad = [
            Vertex {
                position: [1.0, -1.0],
            },
            Vertex {
                position: [1.0, 1.0],
            },
            Vertex {
                position: [-1.0, 1.0],
            },
            Vertex {
                position: [-1.0, 1.0],
            },
            Vertex {
                position: [-1.0, -1.0],
            },
            Vertex {
                position: [1.0, -1.0],
            },
            Vertex {
                position: [0.0, 0.0], // padding
            },
            Vertex {
                position: [0.0, 0.0], // padding
            },
        ];

        let view_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("debug texture view"),
            contents: bytemuck::cast_slice(&[0.0_f32, 0.0, 1.0, 1.0]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("debug_texture"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("quad"),
                            contents: bytemuck::cast_slice(&quad),
                            usage: wgpu::BufferUsages::STORAGE,
                        }),
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: view_uniform_buffer.as_entire_binding(),
                },
            ],
        });

        let texture_bind_groups =
            create_texture_bind_groups(device, &texture_bind_group_layout, texture_views);

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&bind_group_layout), Some(&texture_bind_group_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../shader/texture.wgsl"
            ))),
        });

        let mut pipeline_descriptor = wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(swapchain_format.into())],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: Default::default(),
            cache: None,
        };
        let pipeline = device.create_render_pipeline(&pipeline_descriptor);
        pipeline_descriptor.fragment.as_mut().unwrap().entry_point = Some("fs_scalar");
        let scalar_pipeline = device.create_render_pipeline(&pipeline_descriptor);

        Self {
            _bind_group_layout: bind_group_layout,
            texture_bind_group_layout,
            bind_group,
            view_uniform_buffer,
            texture_bind_groups,
            _sampler: sampler,
            _pipeline_layout: pipeline_layout,
            pipeline,
            scalar_pipeline,
        }
    }

    pub fn set_view_transform(&self, queue: &wgpu::Queue, view: super::ViewTransform) {
        queue.write_buffer(
            &self.view_uniform_buffer,
            0,
            bytemuck::cast_slice(&[view.offset, view.scale]),
        );
    }

    /// Point the named views at new textures, for example after a resize.
    pub fn set_texture_views(
        &mut self,
        device: &wgpu::Device,
        texture_views: &[(&str, &wgpu::TextureView)],
    ) {
        self.texture_bind_groups =
            create_texture_bind_groups(device, &self.texture_bind_group_layout, texture_views);
    }

    pub fn draw_texture<'rpass>(
        &'rpass self,
        _device: &wgpu::Device,
        rpass: &mut wgpu::RenderPass<'rpass>,
        name: &str,
    ) {
        let some_texture_bind_group = self
            .texture_bind_groups
            .iter()
            .find(|(ref n, _)| n == name)
            .map(|(_, bg)| bg);

        if let Some(texture_bind_group) = some_texture_bind_group {
            rpass.set_pipeline(if matches!(name, "pressure" | "divergence") {
                &self.scalar_pipeline
            } else {
                &self.pipeline
            });
            rpass.set_bind_group(0, &self.bind_group, &[]);
            rpass.set_bind_group(1, texture_bind_group, &[]);
            rpass.draw(0..6, 0..1);
        }
    }
}

fn create_texture_bind_groups(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    texture_views: &[(&str, &wgpu::TextureView)],
) -> Vec<(String, wgpu::BindGroup)> {
    texture_views
        .iter()
        .map(|(name, texture_view)| {
            (
                name.to_string(),
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("texture"),
                    layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(texture_view),
                    }],
                }),
            )
        })
        .collect()
}
