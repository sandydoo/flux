use crate::{drawer, fluid, noise, render, rng, settings};
use drawer::Drawer;
use fluid::Fluid;
use noise::{NoiseGenerator, NoiseGeneratorBuilder};
use settings::Settings;

use glow::HasContext;
use std::fmt;
use std::rc::Rc;

// The time at which the animation timer will reset to zero.
const MAX_ELAPSED_TIME: f32 = 1000.0;
const MAX_FRAME_TIME: f32 = 1.0 / 10.0;

pub struct Flux {
    context: render::Context,
    fluid: Fluid,
    drawer: Drawer,
    noise_generator: NoiseGenerator,
    settings: Rc<Settings>,

    // A timestamp in milliseconds. Either host or video time.
    last_timestamp: f64,

    // A local animation timer in seconds that resets at MAX_ELAPSED_TIME.
    elapsed_time: f32,

    fluid_update_interval: f32,
    fluid_frame_time: f32,
}

impl Flux {
    pub fn update(&mut self, settings: &Rc<Settings>) {
        self.settings = Rc::clone(settings);
        self.fluid.update(&self.settings);
        self.drawer.update(&self.settings);
        self.noise_generator.update(&self.settings.noise_channels);

        self.fluid_update_interval = 1.0 / settings.fluid_frame_rate;
    }

    pub fn sample_colors_from_image(&mut self, encoded_bytes: &[u8]) {
        if let Err(msg) = self.drawer.set_color_texture(encoded_bytes) {
            log::error!("{}", msg);
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn sample_colors_from_image_bitmap(&mut self, bitmap: &web_sys::ImageBitmap) {
        if let Err(msg) = self.drawer.set_color_texture_from_image_bitmap(bitmap) {
            log::error!("{}", msg);
        }
    }

    pub fn new(
        context: &render::Context,
        logical_width: u32,
        logical_height: u32,
        physical_width: u32,
        physical_height: u32,
        settings: &Rc<Settings>,
    ) -> Result<Flux, Problem> {
        log::info!("✨ Initialising Flux");

        rng::init_from_seed(&settings.seed);

        let drawer = Drawer::new(
            context,
            logical_width,
            logical_height,
            physical_width,
            physical_height,
            settings,
        )
        .map_err(Problem::Render)?;

        let fluid =
            Fluid::new(context, drawer.scaling_ratio(), settings).map_err(Problem::Render)?;

        let mut noise_generator_builder =
            NoiseGeneratorBuilder::new(context, 2 * settings.fluid_size, drawer.scaling_ratio());
        settings.noise_channels.iter().for_each(|channel| {
            noise_generator_builder.add_channel(channel);
        });
        let noise_generator = noise_generator_builder.build().map_err(Problem::Render)?;

        Ok(Flux {
            fluid,
            drawer,
            noise_generator,
            settings: Rc::clone(settings),

            context: Rc::clone(context),
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
        self.drawer
            .resize(
                logical_width,
                logical_height,
                physical_width,
                physical_height,
            )
            .unwrap();
        self.fluid.resize(self.drawer.scaling_ratio()).unwrap();
        self.noise_generator
            .resize(2 * self.settings.fluid_size, self.drawer.scaling_ratio())
            .unwrap();
    }

    pub fn animate(&mut self, timestamp: f64) {
        self.compute(timestamp);
        self.render();
    }

    pub fn compute(&mut self, timestamp: f64) {
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
            self.noise_generator.generate(self.elapsed_time);

            self.fluid.advect_forward(self.settings.fluid_timestep);
            self.fluid.advect_reverse(self.settings.fluid_timestep);
            self.fluid.adjust_advection(self.settings.fluid_timestep);
            self.fluid.diffuse(self.settings.fluid_timestep);

            self.noise_generator.blend_noise_into(
                self.fluid.get_velocity_textures(),
                self.settings.fluid_timestep,
            );

            self.fluid.calculate_divergence();
            self.fluid.solve_pressure();
            self.fluid.subtract_gradient();

            self.fluid_frame_time -= self.fluid_update_interval;
        }

        // TODO: the line animation is still dependent on the client’s fps. Is
        // this worth fixing?
        self.drawer
            .place_lines(&self.fluid.get_velocity(), self.elapsed_time, timestep);
    }

    pub fn render(&self) {
        unsafe {
            self.context.clear_color(0.0, 0.0, 0.0, 1.0);
            self.context.clear(glow::COLOR_BUFFER_BIT);
        }

        use settings::Mode::*;
        match &self.settings.mode {
            Normal => {
                self.drawer.draw_lines();
                self.drawer.draw_endpoints();
            }
            DebugNoise => {
                self.drawer.draw_texture(self.noise_generator.get_noise());
            }
            DebugFluid => {
                self.drawer.draw_texture(&self.fluid.get_velocity());
            }
            DebugPressure => {
                self.drawer.draw_texture(&self.fluid.get_pressure());
            }
            DebugDivergence => {
                self.drawer.draw_texture(self.fluid.get_divergence());
            }
        };
    }
}

#[derive(Debug)]
pub enum Problem {
    ReadSettings(String),
    ReadImage(std::io::Error),
    DecodeColorTexture(image::ImageError),
    Render(render::Problem),
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::ReadSettings(msg) => write!(f, "{}", msg),
            Problem::ReadImage(msg) => write!(f, "{}", msg),
            Problem::DecodeColorTexture(msg) => write!(f, "Failed to decode image: {}", msg),
            Problem::Render(render_msg) => write!(f, "{}", render_msg),
        }
    }
}
