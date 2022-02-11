use crate::{drawer, fluid, noise, render, settings};
use drawer::Drawer;
use fluid::Fluid;
use noise::NoiseInjector;
use settings::Settings;

use glow::HasContext;
use std::fmt;
use std::rc::Rc;

pub struct Flux {
    fluid: Fluid,
    drawer: Drawer,
    noise_injector: NoiseInjector,
    settings: Rc<Settings>,

    pub context: render::Context,
    elapsed_time: f32,
    last_timestamp: f32,
    frame_time: f32,
    fluid_frame_time: f32,
    max_frame_time: f32,
}

impl Flux {
    pub fn update(&mut self, settings: &Rc<Settings>) -> () {
        self.settings = Rc::clone(settings);

        self.fluid.update(&self.settings);
        self.drawer.update(&self.settings);
        self.noise_injector
            .update_channel(0, &self.settings.noise_channel_1);
        self.noise_injector
            .update_channel(1, &self.settings.noise_channel_2);
    }

    pub fn new(
        context: &render::Context,
        logical_width: u32,
        logical_height: u32,
        physical_width: u32,
        physical_height: u32,
        settings: &Rc<Settings>,
    ) -> Result<Flux, Problem> {
        let fluid_frame_time = 1.0 / settings.fluid_simulation_frame_rate;
        let ratio = logical_width as f32 / logical_height as f32;
        let fluid = Fluid::new(&context, ratio, &settings).map_err(Problem::CannotRender)?;

        let drawer = Drawer::new(
            &context,
            logical_width,
            logical_height,
            physical_width,
            physical_height,
            &settings,
        )
        .map_err(Problem::CannotRender)?;

        let mut noise_injector = NoiseInjector::new(&context, fluid.width / 4, fluid.height / 4)
            .map_err(Problem::CannotRender)?;

        noise_injector
            .add_noise(settings.noise_channel_1.clone())
            .map_err(Problem::CannotRender)?;
        noise_injector
            .add_noise(settings.noise_channel_2.clone())
            .map_err(Problem::CannotRender)?;

        Ok(Flux {
            fluid,
            drawer,
            noise_injector,
            settings: Rc::clone(settings),

            context: Rc::clone(context),
            elapsed_time: 0.0,
            last_timestamp: 0.0,
            frame_time: 0.0,
            fluid_frame_time,
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
        let ratio = logical_width as f32 / logical_height as f32;
        self.fluid.resize(ratio).unwrap(); // fix

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
            .min(0.001 * (timestamp - self.last_timestamp));
        self.last_timestamp = timestamp;
        self.elapsed_time += timestep;
        self.frame_time += timestep;

        while self.frame_time >= self.fluid_frame_time {
            self.noise_injector.generate_all(self.elapsed_time);
            self.noise_injector
                .blend_noise_into(&self.fluid.get_velocity_textures(), self.elapsed_time);

            self.fluid.prepare_pass(self.fluid_frame_time);
            self.fluid.advect();
            self.fluid.diffuse(self.fluid_frame_time); // <- Convection
            self.fluid.calculate_divergence();
            self.fluid.solve_pressure();
            self.fluid.subtract_gradient();

            self.frame_time -= self.fluid_frame_time;
        }

        // TODO: the line animation is still dependent on the client’s fps. Is
        // this worth fixing?
        self.drawer
            .place_lines(timestep, &self.fluid.get_velocity());

        self.drawer.with_antialiasing(|| unsafe {
            // unsafe {
            self.context.clear_color(0.0, 0.0, 0.0, 1.0);
            self.context.clear(glow::COLOR_BUFFER_BIT);

            // Debugging
            // self.drawer.draw_texture(self.noise_injector.get_noise_channel(0).unwrap());
            // self.drawer.draw_texture(self.noise_injector.get_noise_channel(1).unwrap());
            // self.drawer.draw_texture(&self.fluid.get_velocity());
            // self.drawer.draw_texture(&self.fluid.get_pressure());

            self.drawer.draw_lines();
            self.drawer.draw_endpoints();
            // }
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
