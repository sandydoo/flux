use std::sync::{mpsc, Arc};

use flux::{BackendCaps, Flux, Settings};

struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

#[test]
#[ignore = "requires a GPU adapter"]
fn scalar_debug_views_reveal_signed_changes() {
    use flux::settings::Mode::{DebugDivergence, DebugPressure};

    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    // R32Float previews must also work without FLOAT32_FILTERABLE.
    let renderer = Renderer::new(&adapter, false);
    let mut settings = Arc::new(Settings {
        seed: Some("scalar debug visibility".into()),
        ..Default::default()
    });
    let mut flux = Flux::new(
        &renderer.device,
        &renderer.queue,
        wgpu::TextureFormat::Rgba8Unorm,
        640,
        400,
        640,
        400,
        BackendCaps {
            float32_filterable: false,
        },
        &settings,
    )
    .unwrap();
    for mode in [DebugPressure, DebugDivergence] {
        Arc::make_mut(&mut settings).mode = mode;
        flux.update(&renderer.device, &renderer.queue, &settings);
        assert!(renderer
            .snapshot(&flux, 640, 400)
            .chunks_exact(4)
            .all(|p| { p[0].abs_diff(128) <= 1 && p[0] == p[1] && p[1] == p[2] && p[3] == 255 }));
    }
    let mut timestamp = 0.0;
    renderer.advance(&mut flux, &mut timestamp, 30);
    for mode in [DebugPressure, DebugDivergence] {
        Arc::make_mut(&mut settings).mode = mode;
        flux.update(&renderer.device, &renderer.queue, &settings);
        let before = renderer.snapshot(&flux, 640, 400);
        assert!(before
            .chunks_exact(4)
            .any(|p| i16::from(p[0]) - i16::from(p[2]) > 30));
        assert!(before
            .chunks_exact(4)
            .any(|p| i16::from(p[2]) - i16::from(p[0]) > 30));
        renderer.advance(&mut flux, &mut timestamp, 10);
        assert_ne!(before, renderer.snapshot(&flux, 640, 400));
    }
}

impl Renderer {
    fn new(adapter: &wgpu::Adapter, filterable: bool) -> Self {
        let mut features = wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
        if filterable {
            features |= wgpu::Features::FLOAT32_FILTERABLE;
        }
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_features: features,
            required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .unwrap();
        Self { device, queue }
    }

    fn advance(&self, flux: &mut Flux, timestamp: &mut f64, frames: u32) {
        for _ in 0..frames {
            *timestamp += 1000.0 / 60.0;
            let mut encoder = self.device.create_command_encoder(&Default::default());
            flux.compute(&self.device, &self.queue, &mut encoder, *timestamp);
            self.queue.submit([encoder.finish()]);
        }
    }

    fn snapshot(&self, flux: &Flux, width: u32, height: u32) -> Vec<u8> {
        self.snapshot_viewport(flux, width, height, None)
    }

    fn snapshot_viewport(
        &self,
        flux: &Flux,
        width: u32,
        height: u32,
        viewport: Option<flux::render::ScreenViewport>,
    ) -> Vec<u8> {
        let output = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scaling regression output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let row_bytes = (width * 4).div_ceil(256) * 256;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scaling regression readback"),
            size: u64::from(row_bytes) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        flux.render(
            &self.device,
            &self.queue,
            &mut encoder,
            &output.create_view(&Default::default()),
            viewport,
        );
        encoder.copy_texture_to_buffer(
            output.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_bytes),
                    rows_per_image: None,
                },
            },
            output.size(),
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        rx.recv().unwrap().unwrap();
        let mapped = readback.slice(..).get_mapped_range().unwrap();
        mapped
            .chunks(row_bytes as usize)
            .flat_map(|row| row[..width as usize * 4].iter().copied())
            .collect()
    }
}

#[test]
#[ignore = "requires a GPU adapter"]
fn debug_textures_follow_line_zoom_and_viewport() {
    use flux::settings::Mode::{DebugDivergence, DebugFluid, DebugNoise, DebugPressure};

    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let renderer = Renderer::new(&adapter, false);
    let mut settings = Arc::new(Settings {
        seed: Some("debug texture scaling".into()),
        view_scale: 1.0,
        ..Default::default()
    });
    let mut flux = Flux::new(
        &renderer.device,
        &renderer.queue,
        wgpu::TextureFormat::Rgba8Unorm,
        640,
        400,
        640,
        400,
        BackendCaps {
            float32_filterable: false,
        },
        &settings,
    )
    .unwrap();
    renderer.advance(&mut flux, &mut 0.0, 30);
    for mode in [DebugNoise, DebugFluid, DebugPressure, DebugDivergence] {
        Arc::make_mut(&mut settings).mode = mode.clone();
        Arc::make_mut(&mut settings).view_scale = 1.0;
        flux.update(&renderer.device, &renderer.queue, &settings);
        // Power-of-two output keeps nearest samples away from texel boundaries.
        let original = renderer.snapshot(&flux, 512, 512);
        assert!(original.chunks_exact(4).any(|p| p != &original[..4]));
        let crop = |left: usize, top: usize| -> Vec<u8> {
            (top..top + 256)
                .flat_map(|y| {
                    original[(y * 512 + left) * 4..(y * 512 + left + 256) * 4]
                        .iter()
                        .copied()
                })
                .collect()
        };
        Arc::make_mut(&mut settings).view_scale = 2.0;
        flux.update(&renderer.device, &renderer.queue, &settings);
        // Double zoom maps the central half of the field to the whole output.
        assert!(
            renderer.snapshot(&flux, 256, 256) == crop(128, 128),
            "zoom: {mode:?}"
        );

        Arc::make_mut(&mut settings).view_scale = 1.0;
        flux.update(&renderer.device, &renderer.queue, &settings);
        let viewport = flux::render::ScreenViewport::new(320, 200, 320, 200);
        assert!(
            renderer.snapshot_viewport(&flux, 256, 256, Some(viewport)) == crop(256, 256),
            "viewport: {mode:?}",
        );
    }
}

/// Run explicitly on a machine with a GPU. Tests the actual shaders and
/// resampled line state, with simulation frozen while comparing raster sizes.
#[test]
#[ignore = "requires a GPU adapter"]
fn frozen_lines_preserve_logical_size_across_span_and_backing_changes() {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    for filterable in [false, true] {
        if filterable
            && !adapter
                .features()
                .contains(wgpu::Features::FLOAT32_FILTERABLE)
        {
            continue;
        }
        let renderer = Renderer::new(&adapter, filterable);
        let mut settings = Arc::new(Settings {
            seed: Some("render scaling regression".into()),
            line_length: 80.0,
            line_width: 12.0,
            noise_multiplier: 3.0,
            ..Default::default()
        });
        let mut flux = Flux::new(
            &renderer.device,
            &renderer.queue,
            wgpu::TextureFormat::Rgba8Unorm,
            640,
            400,
            640,
            400,
            BackendCaps {
                float32_filterable: filterable,
            },
            &settings,
        )
        .unwrap();
        let mut timestamp = 0.0;
        renderer.advance(&mut flux, &mut timestamp, 90);
        let original = renderer.snapshot(&flux, 640, 400);
        assert!(
            original
                .chunks_exact(4)
                .any(|pixel| pixel[0] > 20 || pixel[1] > 20 || pixel[2] > 20),
            "comparison must contain visible lines"
        );

        flux.resize(&renderer.device, &renderer.queue, 1280, 400, 1280, 400);
        let wide = renderer.snapshot(&flux, 1280, 400);
        let mut total_error = 0u64;
        let mut samples = 0u64;
        // Ignore the edges, where newly revealed lines can extend into view.
        for y in 80..320usize {
            for x in 160..480usize {
                for channel in 0..3 {
                    total_error += original[(y * 640 + x) * 4 + channel]
                        .abs_diff(wide[(y * 1280 + x + 320) * 4 + channel])
                        as u64;
                    samples += 1;
                }
            }
        }
        let error = total_error as f64 / samples as f64;
        assert!(
            error < 0.2,
            "logical span changed frozen lines: mean byte error {error}"
        );

        flux.resize(&renderer.device, &renderer.queue, 1280, 400, 2560, 800);
        let retina = renderer.snapshot(&flux, 2560, 800);
        let mut total_error = 0u64;
        for y in 0..400usize {
            for x in 0..1280usize {
                for channel in 0..3 {
                    let mut sum = 0u32;
                    for dy in 0..2 {
                        for dx in 0..2 {
                            sum += retina[((2 * y + dy) * 2560 + 2 * x + dx) * 4 + channel] as u32;
                        }
                    }
                    total_error +=
                        wide[(y * 1280 + x) * 4 + channel].abs_diff((sum / 4) as u8) as u64;
                }
            }
        }
        let error = total_error as f64 / (1280.0 * 400.0 * 3.0);
        assert!(
            error < 3.0,
            "DPR changed logical geometry: mean byte error {error}"
        );

        // Zero-size callbacks preserve the last valid scene.
        flux.resize(&renderer.device, &renderer.queue, 0, 0, 0, 0);
        assert_eq!(renderer.snapshot(&flux, 2560, 800), retina);

        // Exercise settings changes through Flux, including world-scale updates.
        Arc::make_mut(&mut settings).overall_scale = 1.5;
        Arc::make_mut(&mut settings).fluid_size = 256;
        flux.update(&renderer.device, &renderer.queue, &settings);
        renderer.advance(&mut flux, &mut timestamp, 4);
        let resized = renderer.snapshot(&flux, 2560, 800);
        assert!(resized.chunks_exact(4).any(|pixel| pixel[0] > 20));
    }
}

#[test]
#[ignore = "requires a GPU adapter"]
fn batched_fluid_ticks_match_separately_submitted_ticks() {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    for filterable in [false, true] {
        if filterable
            && !adapter
                .features()
                .contains(wgpu::Features::FLOAT32_FILTERABLE)
        {
            continue;
        }
        let renderer = Renderer::new(&adapter, filterable);
        let settings = Arc::new(Settings {
            seed: Some("batched fluid ticks".into()),
            mode: flux::settings::Mode::DebugFluid,
            // Binary-exact intervals avoid testing timestamp-rounding instead
            // of GPU command ordering. Six ticks fit inside the 100ms clamp.
            fluid_frame_rate: 64.0,
            fluid_timestep: 1.0 / 64.0,
            noise_multiplier: 8.0,
            ..Default::default()
        });
        let simulate = |batched: bool| {
            let mut flux = Flux::new(
                &renderer.device,
                &renderer.queue,
                wgpu::TextureFormat::Rgba8Unorm,
                640,
                400,
                640,
                400,
                BackendCaps {
                    float32_filterable: filterable,
                },
                &settings,
            )
            .unwrap();
            let timestamps = if batched {
                vec![6.0 * 1000.0 / 64.0]
            } else {
                (1..=6)
                    .map(|tick| f64::from(tick) * 1000.0 / 64.0)
                    .collect()
            };
            for timestamp in timestamps {
                let mut encoder = renderer.device.create_command_encoder(&Default::default());
                flux.compute(&renderer.device, &renderer.queue, &mut encoder, timestamp);
                renderer.queue.submit([encoder.finish()]);
            }
            renderer.snapshot(&flux, 640, 400)
        };
        let separate = simulate(false);
        let batched = simulate(true);
        assert!(separate
            .chunks_exact(4)
            .any(|pixel| pixel[0] > 10 || pixel[1] > 10));
        let differing_bytes = batched
            .iter()
            .zip(&separate)
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(
            differing_bytes, 0,
            "fluid tick batching changed the simulation, float32_filterable={filterable}"
        );
    }
}
