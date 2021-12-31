mod data;
mod drawer;
mod fluid;
mod noise;
mod render;
mod settings;
mod web;

use drawer::Drawer;
use fluid::Fluid;
use noise::Noise;
use settings::Settings;
use web::{Canvas, ContextOptions};

use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::WebGl2RenderingContext as GL;

#[wasm_bindgen]
pub struct Flux {
    fluid: Fluid,
    drawer: Drawer,
    noise: Noise,
    settings: Rc<Settings>,

    context: Rc<GL>,
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
        self.fluid.update_settings(&self.settings);
        self.drawer.update_settings(&self.settings);
    }

    #[wasm_bindgen(constructor)]
    pub fn new(settings_object: &JsValue) -> Flux {
        let (context, width, height) = get_rendering_context().unwrap();

        let settings = Rc::new(settings_object.into_serde().unwrap());

        // Settings
        let fluid_simulation_fps: f32 = 15.0;
        let fluid_frame_time: f32 = 1.0 / fluid_simulation_fps;

        let grid_spacing: u32 = 12;
        let view_scale: f32 = 1.4;

        // TODO: deal with result
        let fluid = Fluid::new(&context, &settings).unwrap();

        let mut noise = Noise::new(
            &context,
            2 * settings.fluid_width,
            2 * settings.fluid_height,
        )
        .unwrap();

        let drawer =
            Drawer::new(&context, width, height, &settings, grid_spacing, view_scale).unwrap();

        noise.generate(0.0);
        noise.blend_noise_into(&fluid.get_velocity_textures(), fluid_frame_time);
        // Finish setup before running the main rendering loop
        context.flush();

        Flux {
            fluid,
            drawer,
            noise,
            settings,

            context,
            elapsed_time: 0.0,
            last_timestamp: 0.0,
            frame_time: 0.0,
            fluid_frame_time,
            max_frame_time: 1.0 / 10.0,
        }
    }

    pub fn animate(&mut self, timestamp: f32) {
        self.context.clear_color(0.0, 0.0, 0.0, 1.0);
        self.context.clear(GL::COLOR_BUFFER_BIT);

        let timestep = self
            .max_frame_time
            .min(0.001 * (timestamp - self.last_timestamp));
        self.last_timestamp = timestamp;
        self.elapsed_time += timestep;
        self.frame_time += timestep;

        while self.frame_time >= self.fluid_frame_time {
            self.noise.generate(self.elapsed_time);

            // Convection
            self.fluid.advect(self.fluid_frame_time);

            self.noise
                .blend_noise_into(&self.fluid.get_velocity_textures(), self.fluid_frame_time);

            self.fluid.diffuse(self.fluid_frame_time);

            // TODO: this needs a second pass. See GPU Gems.
            // fluid.curl(fluid_frame_time);

            self.fluid.calculate_divergence();
            self.fluid.solve_pressure();
            self.fluid.subtract_gradient();

            self.frame_time -= self.fluid_frame_time;
        }

        // Debugging
        // drawer.draw_texture(&noise.get_noise());
        // drawer.draw_texture(&fluid.get_velocity());
        // drawer.draw_texture(&fluid.get_pressure());

        // TODO: the line animation is still dependent on the client’s fps. Is
        // this worth fixing?
        self.drawer.place_lines(
            timestep * self.settings.adjust_advection,
            &self.fluid.get_velocity(),
        );
        self.drawer.draw_lines();
        self.drawer.draw_endpoints();
    }
}

fn get_rendering_context() -> Result<(Rc<GL>, u32, u32), JsValue> {
    web::set_panic_hook();

    let window = web::window();
    let document = window.document().unwrap();
    let html_canvas = document.get_element_by_id("canvas").unwrap();
    let html_canvas: web_sys::HtmlCanvasElement =
        html_canvas.dyn_into::<web_sys::HtmlCanvasElement>()?;

    let pixel_ratio: f64 = window.device_pixel_ratio();
    let client_width = html_canvas.client_width() as u32;
    let client_height = html_canvas.client_height() as u32;
    let width = (pixel_ratio * (client_width as f64)).floor() as u32;
    let height = (pixel_ratio * (client_height as f64)).floor() as u32;
    html_canvas.set_width(width);
    html_canvas.set_height(height);

    let canvas = Canvas::new(html_canvas);

    let options = ContextOptions {
        // Disabling alpha can lead to poor performance on some platforms.
        // We’ll need it later when implementing MSAA
        alpha: true,
        depth: false,
        stencil: false,
        desynchronized: false,
        antialias: false,
        fail_if_major_performance_caveat: false,
        power_preference: "high-performance",
        premultiplied_alpha: true,
        preserve_drawing_buffer: false,
    }
    .serialize();

    let gl = canvas
        .get_context_with_context_options("webgl2", &options)?
        .unwrap()
        .dyn_into::<GL>()?;
    gl.get_extension("OES_texture_float")?;
    gl.get_extension("OES_texture_float_linear")?;
    gl.get_extension("EXT_color_buffer_float")?;
    gl.get_extension("EXT_float_blend")?;

    gl.disable(GL::BLEND);
    gl.disable(GL::DEPTH_TEST);

    Ok((Rc::new(gl), width, height))
}
