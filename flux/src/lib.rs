mod data;
mod drawer;
mod fluid;
mod noise;
mod render;
mod settings;
mod web;

use drawer::Drawer;
use fluid::Fluid;
use noise::NoiseInjector;
use settings::Settings;

use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::WebGl2RenderingContext as GL;

#[wasm_bindgen]
pub struct Flux {
    fluid: Fluid,
    drawer: Drawer,
    noise_injector: NoiseInjector,
    settings: Rc<Settings>,

    context: render::Context,
    canvas: web::Canvas,
    width: u32,
    height: u32,
    pixel_ratio: f64,
    elapsed_time: f32,
    last_timestamp: f32,
    frame_time: f32,
    fluid_frame_time: f32,
    max_frame_time: f32,
}

#[wasm_bindgen]
impl Flux {
    #[wasm_bindgen(setter)]
    pub fn set_settings(&mut self, settings_object: &JsValue) -> () {
        let new_settings: Settings = settings_object.into_serde().unwrap();
        self.settings = Rc::new(new_settings);

        // self.fluid.update_settings(&self.settings);
        self.drawer.update_settings(&self.settings);
        // self.noise_injector
        //     .update_channel(0, self.settings.noise_channel_1.clone());
        // self.noise_injector
        //     .update_channel(1, self.settings.noise_channel_2.clone());
    }

    #[wasm_bindgen(constructor)]
    pub fn new(settings_object: &JsValue) -> Result<Flux, JsValue> {
        let (canvas, context, width, height, pixel_ratio) = web::get_rendering_context("canvas")?;
        let context = Rc::new(context);

        let settings: Rc<Settings> = match settings_object.into_serde() {
            Ok(settings) => Rc::new(settings),
            Err(msg) => return Err(JsValue::from_str(&msg.to_string())),
        };

        let fluid_frame_time = 1.0 / settings.fluid_simulation_frame_rate;
        let fluid = Fluid::new(&context, &settings).map_err(|msg| msg.to_string())?;

        let drawer =
            Drawer::new(&context, width, height, &settings).map_err(|msg| msg.to_string())?;

        let mut noise_injector =
            NoiseInjector::new(&context, settings.fluid_width, settings.fluid_height)
                .map_err(|msg| msg.to_string())?;

        noise_injector
            .add_noise(settings.noise_channel_1.clone())
            .map_err(|msg| msg.to_string())?;
        noise_injector
            .add_noise(settings.noise_channel_2.clone())
            .map_err(|msg| msg.to_string())?;

        noise_injector.generate_by_channel_number(0, 0.0);
        context.flush();

        Ok(Flux {
            fluid,
            drawer,
            noise_injector,
            settings,

            context,
            canvas,
            width,
            height,
            pixel_ratio,
            elapsed_time: 0.0,
            last_timestamp: 0.0,
            frame_time: 0.0,
            fluid_frame_time,
            max_frame_time: 1.0 / 10.0,
        })
    }

    pub fn animate(&mut self, timestamp: f32) {
        let new_width = (self.pixel_ratio * f64::from(self.canvas.client_width())) as u32;
        let new_height = (self.pixel_ratio * f64::from(self.canvas.client_height())) as u32;

        if (self.width != new_width) || (self.height != new_height) {
            self.canvas.set_width(new_width);
            self.canvas.set_height(new_height);
            self.drawer.resize(new_width, new_height);
        }

        let timestep = self
            .max_frame_time
            .min(0.001 * (timestamp - self.last_timestamp));
        self.last_timestamp = timestamp;
        self.elapsed_time += timestep;
        self.frame_time += timestep;

        self.noise_injector.generate_all(self.elapsed_time);
        self.noise_injector
            .blend_noise_into(&self.fluid.get_velocity_textures(), self.elapsed_time);

        while self.frame_time >= self.fluid_frame_time {
            self.fluid.advect(self.fluid_frame_time);
            // Convection
            self.fluid.diffuse(self.fluid_frame_time);
            self.fluid.calculate_divergence();
            self.fluid.solve_pressure();
            self.fluid.subtract_gradient();

            self.frame_time -= self.fluid_frame_time;
        }

        // TODO: the line animation is still dependent on the client’s fps. Is
        // this worth fixing?
        self.drawer.place_lines(
            timestep * self.settings.adjust_advection,
            &self.fluid.get_velocity(),
        );

        self.drawer.with_antialiasing(|| {
            self.context.clear_color(0.0, 0.0, 0.0, 1.0);
            self.context.clear(GL::COLOR_BUFFER_BIT);

            // Debugging
            // self.drawer
            //     .draw_texture(self.noise_injector.get_noise_channel(0).unwrap());
            // self.drawer.draw_texture(self.noise_injector.get_noise_channel(1).unwrap());
            // self.drawer.draw_texture(&self.fluid.get_velocity());
            // self.drawer.draw_texture(&self.fluid.get_pressure());

            self.drawer.draw_lines();
            self.drawer.draw_endpoints();
        });
    }
}
