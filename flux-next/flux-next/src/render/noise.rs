use crate::{grid, rng, settings};

use std::borrow::Cow;
use wgpu::util::DeviceExt;

pub struct NoiseGenerator {
    channels: Vec<NoiseChannel>,
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    scaling_ratio: grid::ScalingRatio,

    channel_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,

    generate_noise_pipeline: wgpu::ComputePipeline,
    // inject_noise_pipeline: wgpu::ComputePipeline,

    // noise_buffer: VertexArrayObject,
    // uniforms: UniformBlock<UniformArray<NoiseUniforms>>,
}

impl NoiseGenerator {
    pub fn resize(&mut self, size: u32, scaling_ratio: grid::ScalingRatio) {
        if scaling_ratio == self.scaling_ratio {
            return;
        }

        // self.scaling_ratio = scaling_ratio;
        // let (width, height) = (
        //     size * self.scaling_ratio.rounded_x(),
        //     size * self.scaling_ratio.rounded_y(),
        // );
        // self.texture = Framebuffer::new(
        //     &self.context,
        //     width,
        //     height,
        //     TextureOptions {
        //         mag_filter: glow::LINEAR,
        //         min_filter: glow::LINEAR,
        //         format: glow::RG16F,
        //         ..Default::default()
        //     },
        // )?;
        // self.texture.with_data(None::<&[f16]>)
    }

    pub fn update(&mut self, new_settings: &[settings::Noise]) {
        for (channel, new_setting) in self.channels.iter_mut().zip(new_settings.iter()) {
            channel.settings = new_setting.clone();
        }
    }

    pub fn generate<'cpass>(
        &'cpass self,
        cpass: &mut wgpu::ComputePass<'cpass>,
        elapsed_time: f32,
    ) {
        let workgroup = (
            self.texture.size().width / 8,
            self.texture.size().height / 8,
            1,
        );
        cpass.set_pipeline(&self.generate_noise_pipeline);
        cpass.set_bind_group(0, &self.bind_group, &[]);
        cpass.set_push_constants(0, bytemuck::cast_slice(&[elapsed_time]));
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);

        // self.uniforms
        //     .update(|noise_uniforms| {
        //         *noise_uniforms = UniformArray(
        //             self.channels
        //                 .iter()
        //                 .map(|channel| NoiseUniforms::new(self.scaling_ratio, channel))
        //                 .collect(),
        //         )
        //     })
        //     .buffer_data();

        // self.generate_noise_pass.use_program();

        // unsafe {
        //     self.noise_buffer.bind();
        //     self.uniforms.bind();

        //     self.texture.draw_to(&self.context, || {
        //         self.context.draw_arrays(glow::TRIANGLES, 0, 6);
        //     });
        // }

        // for channel in self.channels.iter_mut() {
        //     channel.tick(elapsed_time);
        // }
    }

    // pub fn blend_noise_into(&mut self, target_textures: &DoubleFramebuffer, timestep: f32) {
    //     target_textures.draw_to(&self.context, |target_texture| {
    //         self.inject_noise_pass.use_program();

    //         unsafe {
    //             self.context.disable(glow::BLEND);
    //             self.noise_buffer.bind();

    //             self.inject_noise_pass.set_uniform(&Uniform {
    //                 name: "deltaTime",
    //                 value: UniformValue::Float(timestep),
    //             });

    //             self.context.active_texture(glow::TEXTURE0);
    //             self.context
    //                 .bind_texture(glow::TEXTURE_2D, Some(target_texture.texture));

    //             self.context.active_texture(glow::TEXTURE1);
    //             self.context
    //                 .bind_texture(glow::TEXTURE_2D, Some(self.texture.texture));

    //             self.context.draw_arrays(glow::TRIANGLES, 0, 6);
    //         }
    //     });
    // }

    pub fn get_noise_texture_view(&self) -> &wgpu::TextureView {
        &self.texture_view
    }
}

pub struct NoiseGeneratorBuilder {
    size: u32,
    scaling_ratio: grid::ScalingRatio,
    channels: Vec<NoiseChannel>,
}

impl NoiseGeneratorBuilder {
    pub fn new(size: u32, scaling_ratio: grid::ScalingRatio) -> Self {
        NoiseGeneratorBuilder {
            size,
            scaling_ratio,
            channels: Vec::new(),
        }
    }

    pub fn add_channel(&mut self, channel: &settings::Noise) -> &Self {
        self.channels.push(NoiseChannel {
            settings: channel.clone(),
            scale: channel.scale,
            offset_1: 4.0 * rng::gen::<f32>(),
            offset_2: 0.0,
            blend_factor: 0.0,
        });

        self
    }

    pub fn build(self, device: &wgpu::Device, queue: &wgpu::Queue) -> NoiseGenerator {
        log::info!("🎛 Generating noise");

        let (width, height) = (
            self.size * self.scaling_ratio.rounded_x(),
            self.size * self.scaling_ratio.rounded_y(),
        );

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("texture:noise"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // TODO: try RG16Float
            format: wgpu::TextureFormat::Rg32Float,
            view_formats: &[],
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_DST,
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
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

        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sampler:linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("view:noise"),
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniform:NoiseChannels"),
            contents: bytemuck::cast_slice(
                &self
                    .channels
                    .iter()
                    .map(|channel| NoiseUniforms::new(self.scaling_ratio, channel))
                    .collect::<Vec<_>>(),
            ),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Noise bind group layout"),
            entries: &[
                // noiseTexture
                // wgpu::BindGroupLayoutEntry {
                //     binding: 0,
                //     visibility: wgpu::ShaderStages::COMPUTE,
                //     ty: wgpu::BindingType::Texture {
                //         sample_type: wgpu::TextureSampleType::Float { filterable: true },
                //         view_dimension: wgpu::TextureViewDimension::D2,
                //         multisampled: false,
                //     },
                //     count: None,
                // },
                // // sampler
                // wgpu::BindGroupLayoutEntry {
                //     binding: 2,
                //     visibility: wgpu::ShaderStages::COMPUTE,
                //     ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                //     count: None,
                // },
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
                // outTexture
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Noise bind group"),
            layout: &bind_group_layout,
            entries: &[
                // wgpu::BindGroupEntry {
                //     binding: 2,
                //     resource: wgpu::BindingResource::Sampler(&linear_sampler),
                // },
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &uniform_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Noise layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::COMPUTE,
                range: 0..8,
            }],
        });

        let generate_noise_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Generate noise shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../shader/generate_noise.comp.wgsl"
            ))),
        });

        let generate_noise_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Generate noise"),
                layout: Some(&pipeline_layout),
                module: &generate_noise_shader,
                entry_point: "main",
            });

        NoiseGenerator {
            channels: self.channels,
            channel_buffer: uniform_buffer,
            scaling_ratio: self.scaling_ratio,
            texture,
            texture_view,
            bind_group,

            generate_noise_pipeline,
            // inject_noise_pipeline,
        }
    }
}

pub struct NoiseChannel {
    settings: settings::Noise,
    scale: f32,
    offset_1: f32,
    offset_2: f32,
    blend_factor: f32,
}

impl NoiseChannel {
    pub fn tick(&mut self, elapsed_time: f32) {
        const BLEND_THRESHOLD: f32 = 20.0;

        self.scale = self.settings.scale
            * (1.0 + 0.15 * (0.01 * elapsed_time * std::f32::consts::TAU).sin());
        self.offset_1 += self.settings.offset_increment;

        if self.offset_1 > BLEND_THRESHOLD {
            self.blend_factor += self.settings.offset_increment;
            self.offset_2 += self.settings.offset_increment;
        }

        // Reset blending
        if self.blend_factor > 1.0 {
            self.offset_1 = self.offset_2;
            self.offset_2 = 0.0;
            self.blend_factor = 0.0;
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct NoiseUniforms {
    scale: [f32; 2],
    offset_1: f32,
    offset_2: f32,
    blend_factor: f32,
    multiplier: f32,
    padding: [f32; 2],
}

impl NoiseUniforms {
    fn new(scaling_ratio: grid::ScalingRatio, channel: &NoiseChannel) -> Self {
        Self {
            scale: [
                channel.scale * scaling_ratio.x(),
                channel.scale * scaling_ratio.y(),
            ]
            .into(),
            offset_1: channel.offset_1,
            offset_2: channel.offset_2,
            blend_factor: channel.blend_factor,
            multiplier: channel.settings.multiplier,
            padding: [0.0; 2],
        }
    }
}
