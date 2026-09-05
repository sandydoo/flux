use crate::{grid, rng, settings, BackendCaps};

use super::downgrade_float_storage;

use std::borrow::Cow;
use std::sync::Arc;
use wgpu::util::DeviceExt;

pub struct NoiseGenerator {
    elapsed_time: f32, // TODO: reset

    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    texture_format: wgpu::TextureFormat,
    scaling_ratio: grid::ScalingRatio,

    uniforms: NoiseUniforms,

    channel_settings: Vec<settings::Noise>,
    channels: Vec<NoiseChannel>,
    phase: NoisePhase,

    linear_sampler: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
    channel_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    push_constants_buffer: wgpu::Buffer,
    inject_noise_bind_group_layout: wgpu::BindGroupLayout,
    inject_noise_bind_group: wgpu::BindGroup,

    generate_noise_pipeline: wgpu::ComputePipeline,
    inject_noise_pipeline: wgpu::ComputePipeline,
}

impl NoiseGenerator {
    /// Follow the grid to a new size. The noise texture keeps the aspect ratio
    /// of the grid, like the fluid it is injected into. The noise is generated
    /// again every fluid step, so nothing needs to be carried over.
    pub fn resize(&mut self, device: &wgpu::Device, size: u32, scaling_ratio: grid::ScalingRatio) {
        self.scaling_ratio = scaling_ratio;

        let size = scaling_ratio.texture_size(
            size,
            grid::TextureBudget::for_base(size, device.limits().max_texture_dimension_2d),
        );
        if size == self.texture.size() {
            return;
        }

        let (texture, texture_view) = create_texture(device, &size, self.texture_format);
        self.texture = texture;
        self.texture_view = texture_view;
        self.bind_group = create_bind_group(
            device,
            &self.bind_group_layout,
            &self.uniform_buffer,
            &self.channel_buffer,
            &self.texture_view,
        );
        self.inject_noise_bind_group = create_inject_noise_bind_group(
            device,
            &self.inject_noise_bind_group_layout,
            &self.push_constants_buffer,
            &self.texture_view,
            &self.linear_sampler,
        );
    }

    pub fn update(&mut self, new_settings: &settings::Settings) {
        self.uniforms.multiplier = new_settings.noise_multiplier;
        self.channel_settings = new_settings.noise_channels.to_vec();
    }

    pub fn update_buffers(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        timestep: f32,
    ) {
        self.elapsed_time += timestep;
        self.phase.tick();

        let scaling_ratio = self.scaling_ratio;
        let elapsed_time = self.elapsed_time;
        let base_scale = self
            .channel_settings
            .first()
            .map_or(1.0, |channel| channel.scale);
        self.channels
            .iter_mut()
            .zip(self.channel_settings.iter())
            .for_each(|(channel, channel_settings)| {
                channel.tick(
                    scaling_ratio,
                    channel_settings,
                    elapsed_time,
                    &self.phase,
                    base_scale,
                );
            });

        // Queue writes all run before the next submitted command buffer.
        // Immutable snapshots copied in command order let catch-up ticks use
        // their own noise state when several ticks share one submission.
        let mut snapshot = Vec::with_capacity(32 + std::mem::size_of_val(self.channels.as_slice()));
        snapshot.extend_from_slice(bytemuck::cast_slice(&[0.0, 0.0, 0.0, timestep]));
        snapshot.extend_from_slice(bytemuck::bytes_of(&self.uniforms));
        snapshot.extend_from_slice(bytemuck::cast_slice(&self.channels));
        let upload = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("upload:noise_tick"),
            contents: &snapshot,
            usage: wgpu::BufferUsages::COPY_SRC,
        });
        encoder.copy_buffer_to_buffer(&upload, 0, &self.push_constants_buffer, 0, 16);
        encoder.copy_buffer_to_buffer(&upload, 16, &self.uniform_buffer, 0, 16);
        let channel_bytes = snapshot.len() as u64 - 32;
        if channel_bytes > 0 {
            encoder.copy_buffer_to_buffer(&upload, 32, &self.channel_buffer, 0, channel_bytes);
        }
    }

    pub fn generate<'cpass>(&'cpass self, cpass: &mut wgpu::ComputePass<'cpass>) {
        let workgroup = (
            self.texture.size().width.div_ceil(16),
            self.texture.size().height.div_ceil(16),
            1,
        );
        cpass.set_pipeline(&self.generate_noise_pipeline);
        cpass.set_bind_group(0, &self.bind_group, &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
    }

    pub fn inject_noise_into<'cpass>(
        &'cpass self,
        cpass: &mut wgpu::ComputePass<'cpass>,
        target_texture_bind_group: &'cpass wgpu::BindGroup,
        target_texture_size: wgpu::Extent3d,
    ) {
        let workgroup = (
            target_texture_size.width.div_ceil(16),
            target_texture_size.height.div_ceil(16),
            1,
        );
        cpass.set_pipeline(&self.inject_noise_pipeline);
        cpass.set_bind_group(0, &self.inject_noise_bind_group, &[]);
        cpass.set_bind_group(1, target_texture_bind_group, &[]);
        cpass.dispatch_workgroups(workgroup.0, workgroup.1, workgroup.2);
    }

    pub fn get_noise_texture_view(&self) -> &wgpu::TextureView {
        &self.texture_view
    }
}

pub struct NoiseGeneratorBuilder {
    settings: Arc<settings::Settings>,
    size: u32,
    scaling_ratio: grid::ScalingRatio,
    channels: Vec<settings::Noise>,
}

impl NoiseGeneratorBuilder {
    // TODO: just provide the final size, no scaling ratio
    pub fn new(
        size: u32,
        scaling_ratio: grid::ScalingRatio,
        settings: &Arc<settings::Settings>,
    ) -> Self {
        NoiseGeneratorBuilder {
            settings: Arc::clone(settings),
            size,
            scaling_ratio,
            channels: Vec::new(),
        }
    }

    pub fn add_channel(&mut self, channel: &settings::Noise) -> &Self {
        self.channels.push(channel.clone());

        self
    }

    pub fn build(
        self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        caps: BackendCaps,
    ) -> NoiseGenerator {
        log::info!("🎛 Generating noise");

        let uniforms = NoiseUniforms::new(&self.settings);
        let phase = NoisePhase::new();
        let base_scale = self.channels.first().map_or(1.0, |channel| channel.scale);
        let channels = self
            .channels
            .iter()
            .map(|channel| NoiseChannel::new(self.scaling_ratio, channel, &phase, base_scale))
            .collect::<Vec<_>>();

        let size = self.scaling_ratio.texture_size(
            self.size,
            grid::TextureBudget::for_base(self.size, device.limits().max_texture_dimension_2d),
        );

        // The noise texture is linearly sampled in `inject_noise.comp.wgsl`;
        // Rg32Float requires `FLOAT32_FILTERABLE`. The shaders only touch the
        // `.xy` channels, so widening the fallback to Rgba16Float is a no-op
        // — see `downgrade_float_storage` for why we can't fall back to the
        // narrower Rg16Float.
        let noise_format = if caps.float32_filterable {
            wgpu::TextureFormat::Rg32Float
        } else {
            wgpu::TextureFormat::Rgba16Float
        };

        let (texture, texture_view) = create_texture(device, &size, noise_format);

        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sampler:linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniform:noise"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let channel_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("storage:noise_channels"),
            contents: bytemuck::cast_slice(&channels),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bind_group_layout:noise"),
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
                // uniforms
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
                // channels
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
                // outTexture
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: noise_format,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = create_bind_group(
            device,
            &bind_group_layout,
            &uniform_buffer,
            &channel_buffer,
            &texture_view,
        );

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout:generate_noise"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let generate_noise_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader:generate_noise"),
            source: wgpu::ShaderSource::Wgsl(downgrade_float_storage(
                include_str!("../../shader/generate_noise.comp.wgsl"),
                caps,
            )),
        });

        let generate_noise_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("pipeline:generate_noise"),
                layout: Some(&pipeline_layout),
                module: &generate_noise_shader,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });

        let push_constants_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("push_constants:noise"),
            contents: bytemuck::cast_slice(&[0.0, 0.0, 0.0, 0.0]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let inject_noise_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Inject noise bind group layout"),
                entries: &[
                    // push_constants
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
                    // noise_texure
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
                    // sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let inject_noise_bind_group_layout_2 =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Inject noise bind group layout 2"),
                entries: &[
                    // velocity_texture
                    // wgpu::BindGroupLayoutEntry {
                    //     binding: 0,
                    //     visibility: wgpu::ShaderStages::COMPUTE,
                    //     ty: wgpu::BindingType::StorageTexture {
                    //         access: wgpu::StorageTextureAccess::ReadOnly,
                    //         format: wgpu::TextureFormat::Rg32Float,
                    //         view_dimension: wgpu::TextureViewDimension::D2,
                    //     },
                    //     count: None,
                    // },
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

        let inject_noise_bind_group = create_inject_noise_bind_group(
            device,
            &inject_noise_bind_group_layout,
            &push_constants_buffer,
            &texture_view,
            &linear_sampler,
        );

        let inject_noise_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Inject noise layout"),
                bind_group_layouts: &[
                    Some(&inject_noise_bind_group_layout),
                    Some(&inject_noise_bind_group_layout_2),
                ],
                immediate_size: 0,
            });

        let inject_noise_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Inject noise shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../shader/inject_noise.comp.wgsl"
            ))),
        });

        let inject_noise_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Inject noise"),
                layout: Some(&inject_noise_pipeline_layout),
                module: &inject_noise_shader,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });

        NoiseGenerator {
            phase,
            elapsed_time: 0.0,

            uniforms,
            channel_settings: self.channels,
            channels,

            linear_sampler,
            uniform_buffer,
            channel_buffer,
            scaling_ratio: self.scaling_ratio,
            texture,
            texture_view,
            texture_format: noise_format,
            bind_group_layout,
            bind_group,
            inject_noise_bind_group_layout,
            inject_noise_bind_group,
            push_constants_buffer,

            generate_noise_pipeline,
            inject_noise_pipeline,
        }
    }
}

fn create_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform_buffer: &wgpu::Buffer,
    channel_buffer: &wgpu::Buffer,
    texture_view: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bind_group:noise"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: uniform_buffer,
                    offset: 0,
                    size: None,
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: channel_buffer,
                    offset: 0,
                    size: None,
                }),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(texture_view),
            },
        ],
    })
}

fn create_inject_noise_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    push_constants_buffer: &wgpu::Buffer,
    texture_view: &wgpu::TextureView,
    linear_sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Inject noise bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: push_constants_buffer,
                    offset: 0,
                    size: None,
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(linear_sampler),
            },
        ],
    })
}

fn create_texture(
    device: &wgpu::Device,
    size: &wgpu::Extent3d,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("texture:noise"),
        size: *size,
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
    });

    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("view:noise"),
        ..Default::default()
    });

    (texture, texture_view)
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct NoiseChannel {
    scale: [f32; 2],   // 0
    offset_1: f32,     // 8
    offset_2: f32,     // 12
    blend_factor: f32, //16
    multiplier: f32,   // 20
    origin: [f32; 2],  // 24: center phase in the tuned noise coordinates
    pair_offset: [f32; 2], // 32: shift the second component before octave scaling
                       // 40 bytes, aligned to 8 bytes.
}

// All octaves sample one moving 3D field and crossfade together. Channel
// controls still specify their own temporal frequency relative to this clock.
struct NoisePhase {
    offset_1: f32,
    offset_2: f32,
    blend_factor: f32,
}

impl NoisePhase {
    const STEP: f32 = 0.001;
    const BLEND_THRESHOLD: f32 = 1000.0;

    fn new() -> Self {
        Self {
            offset_1: Self::BLEND_THRESHOLD * rng::gen::<f32>(),
            offset_2: 0.0,
            blend_factor: 0.0,
        }
    }

    fn tick(&mut self) {
        self.offset_1 += Self::STEP;
        if self.offset_1 > Self::BLEND_THRESHOLD {
            self.blend_factor += Self::STEP;
            self.offset_2 += Self::STEP;
        }
        if self.blend_factor > 1.0 {
            self.offset_1 = self.offset_2;
            self.offset_2 = 0.0;
            self.blend_factor = 0.0;
        }
    }
}

impl NoiseChannel {
    fn new(
        scaling_ratio: grid::ScalingRatio,
        channel_settings: &settings::Noise,
        phase: &NoisePhase,
        base_scale: f32,
    ) -> Self {
        let frequency = channel_settings.scale / base_scale.max(1e-10);
        let temporal_frequency = channel_settings.offset_increment / NoisePhase::STEP;
        Self {
            scale: scaling_ratio
                .simulation_domain()
                .map(|axis| channel_settings.scale * axis),
            offset_1: phase.offset_1 * temporal_frequency,
            offset_2: phase.offset_2 * temporal_frequency,
            blend_factor: phase.blend_factor,
            multiplier: channel_settings.multiplier,
            origin: [0.5 * channel_settings.scale; 2],
            pair_offset: [8.0 * frequency, -8.0 * frequency],
        }
    }

    fn tick(
        &mut self,
        scaling_ratio: grid::ScalingRatio,
        channel_settings: &settings::Noise,
        elapsed_time: f32,
        phase: &NoisePhase,
        base_scale: f32,
    ) {
        // Preserve the square noise profile the presets were tuned against.
        // Larger world domains reveal more of that field around its center.
        let scale = channel_settings.scale
            * (1.0 + 0.15 * (0.01 * elapsed_time * std::f32::consts::TAU).sin());
        self.scale = scaling_ratio.simulation_domain().map(|axis| scale * axis);
        self.origin = [0.5 * scale; 2];
        self.multiplier = channel_settings.multiplier;
        let temporal_frequency = channel_settings.offset_increment / NoisePhase::STEP;
        self.offset_1 = phase.offset_1 * temporal_frequency;
        self.offset_2 = phase.offset_2 * temporal_frequency;
        self.blend_factor = phase.blend_factor;
        let frequency = channel_settings.scale / base_scale.max(1e-10);
        self.pair_offset = [8.0 * frequency, -8.0 * frequency];
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct NoiseUniforms {
    multiplier: f32, // 0
    _padding: [u32; 3],
}

impl NoiseUniforms {
    fn new(settings: &settings::Settings) -> Self {
        Self {
            multiplier: settings.noise_multiplier,
            _padding: [0, 0, 0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_noise_profile_matches_tuned_coordinates_and_keeps_centered_overlap() {
        rng::init_from_seed(&Some("noise reference profile".into()));
        let phase = NoisePhase::new();
        let channels = settings::Settings::default().noise_channels;
        let base_scale = channels[0].scale;
        for settings in channels {
            let reference_domain = grid::ScalingRatio::new(1280, 800);
            let mut reference = NoiseChannel::new(reference_domain, &settings, &phase, base_scale);
            let mut span = reference;
            for time in [0.0, 2.5, 25.0] {
                reference.tick(reference_domain, &settings, time, &phase, base_scale);
                span.tick(
                    grid::ScalingRatio::new(2560, 800),
                    &settings,
                    time,
                    &phase,
                    base_scale,
                );
                let tuned_scale =
                    settings.scale * (1.0 + 0.15 * (0.01 * time * std::f32::consts::TAU).sin());
                for uv in [[0.0, 0.0], [0.25, 0.75], [0.5, 0.5], [1.0, 1.0]] {
                    let coordinates: [f32; 2] = std::array::from_fn(|axis| {
                        reference.scale[axis] * (uv[axis] - 0.5) + reference.origin[axis]
                    });
                    for axis in [0, 1] {
                        assert!(
                            (coordinates[axis] - tuned_scale * uv[axis]).abs() < 0.00001,
                            "a default channel must retain its original spatial profile"
                        );
                    }
                    let spanned_uv = [0.5 + (uv[0] - 0.5) / 2.0, uv[1]];
                    for axis in [0, 1] {
                        let spanning_coordinate =
                            span.scale[axis] * (spanned_uv[axis] - 0.5) + span.origin[axis];
                        assert!(
                            (coordinates[axis] - spanning_coordinate).abs() < 0.00001,
                            "resizing must reveal the same field around its center"
                        );
                    }
                }
            }
        }
    }

    #[test]
    #[ignore = "requires a compute-capable GPU; run with --ignored"]
    fn batched_noise_ticks_match_individually_submitted_ticks() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let settings = Arc::new(settings::Settings::default());

        let run = |batched: bool| {
            rng::init_from_seed(&Some("noise tick ordering".into()));
            let mut builder =
                NoiseGeneratorBuilder::new(64, grid::ScalingRatio::new(1280, 800), &settings);
            for channel in &settings.noise_channels {
                builder.add_channel(channel);
            }
            let mut noise = builder.build(
                &device,
                &queue,
                BackendCaps {
                    float32_filterable: false,
                },
            );
            let size = noise.texture.size();
            let stride = (size.width * 8).div_ceil(256) * 256;
            let frame_bytes = u64::from(stride) * u64::from(size.height);
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("test:noise_tick_readback"),
                size: frame_bytes * 6,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = device.create_command_encoder(&Default::default());
            for tick in 0..6 {
                // Exercise both the channel state and the uniform snapshot.
                noise.uniforms.multiplier = 1.0 + tick as f32 * 0.1;
                noise.update_buffers(&device, &mut encoder, 1.0 / 60.0);
                {
                    let mut pass = encoder.begin_compute_pass(&Default::default());
                    noise.generate(&mut pass);
                }
                encoder.copy_texture_to_buffer(
                    noise.texture.as_image_copy(),
                    wgpu::TexelCopyBufferInfo {
                        buffer: &readback,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: frame_bytes * tick,
                            bytes_per_row: Some(stride),
                            rows_per_image: Some(size.height),
                        },
                    },
                    size,
                );
                if !batched {
                    queue.submit([encoder.finish()]);
                    encoder = device.create_command_encoder(&Default::default());
                }
            }
            queue.submit([encoder.finish()]);
            readback
                .slice(..)
                .map_async(wgpu::MapMode::Read, |result| result.unwrap());
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            let bytes = readback.slice(..).get_mapped_range().unwrap().to_vec();
            assert_ne!(
                &bytes[..frame_bytes as usize],
                &bytes[5 * frame_bytes as usize..]
            );
            bytes
        };

        let batched = run(true);
        let separate = run(false);
        let differing_bytes = batched
            .iter()
            .zip(&separate)
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(
            differing_bytes, 0,
            "catch-up ticks must each use their own noise snapshot"
        );
    }
}
