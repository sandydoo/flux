use crate::{grid, render, rng, settings};
use settings::Settings;

use std::sync::Arc;
use std::sync::Mutex;

// The time at which the animation timer will reset to zero.
const MAX_ELAPSED_TIME: f32 = 1000.0;
const MAX_FRAME_TIME: f32 = 1.0 / 10.0;

pub struct Flux {
    settings: Arc<Settings>,
    screen_size: wgpu::Extent3d,

    grid: grid::Grid,
    fluid: render::fluid::Context,
    lines: render::lines::Context,
    noise_generator: render::noise::NoiseGenerator,
    debug_texture: render::texture::Context,

    color_image: Arc<Mutex<Option<image::RgbaImage>>>,
    color_bind_group: Option<wgpu::BindGroup>,

    // A timestamp in milliseconds. Either host or video time.
    last_timestamp: f64,

    // A local animation timer in seconds that resets at MAX_ELAPSED_TIME.
    elapsed_time: f32,

    fluid_update_interval: f32,
    fluid_frame_time: f32,
}

impl Flux {
    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, settings: &Arc<Settings>) {
        self.settings = Arc::clone(settings);
        self.fluid
            .update(device, queue, self.grid.scaling_ratio, &self.settings);
        self.noise_generator.update(&self.settings.noise_channels);
        self.lines
            .update(device, queue, self.screen_size, &self.grid, &self.settings);

        self.fluid_update_interval = 1.0 / settings.fluid_frame_rate;
    }

    pub fn sample_colors_from_image(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, image: &image::RgbaImage) {
        let texture_view = render::color::load_color_texture(device, queue, image);
        let layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float {
                            filterable: true,
                        },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            });
        self.color_bind_group =
            Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                }],
            }));
        self.lines.color_mode = 2;
        self.lines.update_line_color_mode(device, queue);
    }

    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        swapchain_format: wgpu::TextureFormat,
        logical_width: u32,
        logical_height: u32,
        physical_width: u32,
        physical_height: u32,
        settings: &Arc<Settings>,
    ) -> Result<Flux, String> {
        log::info!("✨ Initialising Flux");

        rng::init_from_seed(&settings.seed);

        let screen_size = wgpu::Extent3d {
            width: physical_width,
            height: physical_height,
            depth_or_array_layers: 1,
        };

        let grid = grid::Grid::new(logical_width, logical_height, settings.grid_spacing);

        let fluid = render::fluid::Context::new(device, queue, grid.scaling_ratio, settings);

        let lines = render::lines::Context::new(
            device,
            queue,
            swapchain_format,
            screen_size,
            &grid,
            settings,
            fluid.get_velocity_texture_view(),
        );

        let mut noise_generator_builder =
            render::noise::NoiseGeneratorBuilder::new(2 * settings.fluid_size, grid.scaling_ratio);
        settings.noise_channels.iter().for_each(|channel| {
            noise_generator_builder.add_channel(channel);
        });
        let noise_generator = noise_generator_builder.build(device, queue);

        let debug_texture = render::texture::Context::new(
            device,
            swapchain_format,
            &[
                ("fluid", fluid.get_velocity_texture_view()),
                ("noise", noise_generator.get_noise_texture_view()),
                ("pressure", fluid.get_pressure_texture_view()),
                ("divergence", fluid.get_divergence_texture_view()),
            ],
        );

        Ok(Flux {
            settings: Arc::clone(settings),
            screen_size,

            fluid,
            grid,
            lines,
            noise_generator,
            debug_texture,
            color_image: Arc::new(Mutex::new(None)),
            color_bind_group: None,

            last_timestamp: 0.0,
            elapsed_time: 0.0,

            fluid_update_interval: 1.0 / settings.fluid_frame_rate,
            fluid_frame_time: 0.0,
        })
    }

    // TODO: handle errors
    pub fn resize(
        &mut self,
        logical_width: u32,
        logical_height: u32,
        physical_width: u32,
        physical_height: u32,
    ) {
        let grid = grid::Grid::new(logical_width, logical_height, self.settings.grid_spacing);

        // TODO: fetch line state from GPU and resample for new grid

        // self.drawer
        //     .resize(
        //         logical_width,
        //         logical_height,
        //         physical_width,
        //         physical_height,
        //     )
        //     .unwrap();
        // self.fluid.resize(self.drawer.scaling_ratio()).unwrap();
        // self.noise_generator
        //     .resize(2 * self.settings.fluid_size, self.drawer.scaling_ratio())
        //     .unwrap();
    }

    pub fn animate(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        timestamp: f64,
    ) {
        self.compute(device, queue, encoder, timestamp);
        self.render(device, queue, encoder, view);
    }

    pub fn compute(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        timestamp: f64,
    ) {
        // The delta time in seconds
        let timestep = f32::min(
            MAX_FRAME_TIME,
            0.001 * (timestamp - self.last_timestamp) as f32,
        );

        self.last_timestamp = timestamp;
        self.elapsed_time += timestep;
        self.fluid_frame_time += timestep;

        // Reset animation timers to avoid precision issues
        let timer_overflow = self.elapsed_time - MAX_ELAPSED_TIME;
        if timer_overflow >= 0.0 {
            self.elapsed_time = timer_overflow;
        }

        while self.fluid_frame_time >= self.fluid_update_interval {
            self.noise_generator
                .update_buffers(queue, self.settings.fluid_timestep);

            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("flux::compute"),
                timestamp_writes: None,
            });

            self.noise_generator.generate(&mut cpass);

            self.fluid.advect_forward(queue, &mut cpass); // 0
            self.fluid.advect_reverse(queue, &mut cpass); // 0
            self.fluid.adjust_advection(&mut cpass); // 0 -> 1
            self.fluid.diffuse(&mut cpass); // 1 -> 0

            let velocity_bind_group = self.fluid.get_velocity_bind_group(0);
            self.noise_generator.inject_noise_into(
                &mut cpass,
                velocity_bind_group,
                self.fluid.get_fluid_size(),
            ); // 0 -> 1

            self.fluid.calculate_divergence(&mut cpass); // 1
            self.fluid.solve_pressure(queue, &mut cpass);
            self.fluid.subtract_gradient(&mut cpass); // 1 -> 0

            self.fluid_frame_time -= self.fluid_update_interval;
        }

        {
            self.lines
                .tick_line_uniforms(device, queue, timestep, self.elapsed_time);

            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("flux::place_lines"),
                timestamp_writes: None,
            });

            self.lines.place_lines(&mut cpass, &self.color_bind_group);
        }
    }

    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
    ) {
        encoder.push_debug_group("render lines");

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("flux::render"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            use settings::Mode::*;
            match &self.settings.mode {
                Normal => {
                    self.lines.draw_lines(&mut rpass);
                    self.lines.draw_endpoints(&mut rpass);
                }
                DebugNoise => {
                    self.debug_texture.draw_texture(device, &mut rpass, "noise");
                }
                DebugFluid => {
                    self.debug_texture.draw_texture(device, &mut rpass, "fluid");
                }
                DebugPressure => {
                    self.debug_texture
                        .draw_texture(device, &mut rpass, "pressure");
                }
                DebugDivergence => {
                    self.debug_texture
                        .draw_texture(device, &mut rpass, "divergence");
                }
            };
        }

        encoder.pop_debug_group();
    }
}

// #[derive(Debug)]
// pub enum Problem {
//     ReadSettings(String),
//     ReadImage(std::io::Error),
//     DecodeColorTexture(image::ImageError),
//     Render(render::Problem),
// }
//
// impl fmt::Display for Problem {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self {
//             Problem::ReadSettings(msg) => write!(f, "{}", msg),
//             Problem::ReadImage(msg) => write!(f, "{}", msg),
//             Problem::DecodeColorTexture(msg) => write!(f, "Failed to decode image: {}", msg),
//             Problem::Render(render_msg) => write!(f, "{}", render_msg),
//         }
//     }
// }
