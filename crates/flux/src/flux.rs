use crate::{drawer, fluid, noise, render, settings};
use drawer::Drawer;
use fluid::Fluid;
use noise::NoiseGenerator;
use settings::Settings;

use glow::HasContext;
use std::fmt;
use std::rc::Rc;

pub struct Flux {
    fluid: Fluid,
    drawer: Drawer,
    noise_generator: NoiseGenerator,
    settings: Rc<Settings>,

    context: render::Context,
    elapsed_time: f32,
    frame_time: f32,
    fluid_frame_time: f32,
    max_frame_time: f32,
}

impl Flux {
    pub fn update(&mut self, settings: &Rc<Settings>) -> () {
        self.settings = Rc::clone(settings);
        self.fluid.update(&self.settings);
        self.drawer.update(&self.settings);
        self.noise_generator.update(&self.settings.noise_channels);
    }

    pub fn new(
        context: &render::Context,
        logical_width: u32,
        logical_height: u32,
        physical_width: u32,
        physical_height: u32,
        settings: &Rc<Settings>,
    ) -> Result<Flux, Problem> {
        let fluid = Fluid::new(&context, &settings).map_err(Problem::CannotRender)?;

        let drawer = Drawer::new(
            &context,
            logical_width,
            logical_height,
            physical_width,
            physical_height,
            &settings,
        )
        .map_err(Problem::CannotRender)?;

        let mut noise_generator_builder = NoiseGenerator::new(&context, 256, 256);
        for channel in settings.noise_channels.iter() {
            noise_generator_builder.add_channel(&channel);
        }
        let noise_generator = noise_generator_builder
            .build()
            .map_err(Problem::CannotRender)?;

        Ok(Flux {
            fluid,
            drawer,
            noise_generator,
            settings: Rc::clone(settings),

            context: Rc::clone(context),
            elapsed_time: 0.0,
            frame_time: 0.0,
            fluid_frame_time: 1.0 / settings.fluid_simulation_frame_rate,
            max_frame_time: 1.0 / 10.0,
        })
    }

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
            .unwrap(); // fix
    }

    pub fn animate(&mut self, timestamp: f32) {
        let timestep = self
            .max_frame_time
            .min(0.001 * (timestamp - self.elapsed_time));
        self.elapsed_time = timestamp;
        self.frame_time += timestep;

        // TODO: move frame times to fluid
        while self.frame_time >= self.fluid_frame_time {
            self.noise_generator.generate(self.elapsed_time);

            self.fluid.advect_forward();
            self.fluid.advect_reverse();
            self.fluid.adjust_advection();
            self.fluid.diffuse(self.fluid_frame_time); // <- Convection

            self.noise_generator
                .blend_noise_into(&self.fluid.get_velocity_textures(), self.fluid_frame_time);

            self.fluid.calculate_divergence();
            self.fluid.solve_pressure();
            self.fluid.subtract_gradient();

            self.frame_time -= self.fluid_frame_time;
        }

        // TODO: the line animation is still dependent on the client’s fps. Is
        // this worth fixing?
        self.drawer
            .place_lines(&self.fluid.get_velocity(), timestep);

        self.drawer.with_antialiasing(|| {
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
                    self.drawer.draw_texture(&self.fluid.get_divergence());
                }
            };
        });
    }
}

#[derive(Debug)]
pub enum Problem {
    CannotReadSettings(String),
    CannotRender(render::Problem),
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Problem::*;
        match self {
            CannotReadSettings(msg) => write!(f, "{}", msg),
            CannotRender(render_msg) => write!(f, "{}", render_msg.to_string()),
        }
    }
}
