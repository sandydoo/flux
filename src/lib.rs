mod data;
mod drawer;
mod fluid;
mod noise;
mod render;
mod web;

use drawer::Drawer;
use fluid::Fluid;
use noise::Noise;
use web::ContextOptions;

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::WebGl2RenderingContext as GL;

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    web::set_panic_hook();

    let window = web::window();
    let document = window.document().unwrap();
    let html_canvas = document.get_element_by_id("canvas").unwrap();
    let html_canvas: web_sys::HtmlCanvasElement =
        html_canvas.dyn_into::<web_sys::HtmlCanvasElement>()?;
    // TODO: should we handle non-standard pixel ratios?
    let pixel_ratio = window.device_pixel_ratio().ceil() as u32;
    let width = pixel_ratio * html_canvas.client_width() as u32;
    let height = pixel_ratio * html_canvas.client_height() as u32;
    html_canvas.set_width(width);
    html_canvas.set_height(height);

    // Get offscreen canvas to decouple ourselves from the DOM.
    // Performance is much better, but the only browser that has implemeted it
    // is Chrome.
    // let canvas = html_canvas.transfer_control_to_offscreen()?;
    let canvas = html_canvas;

    let options = ContextOptions {
        // Disabling alpha can lead to poor performance on some platforms.
        // We’ll need it later when implementing MSAA
        alpha: true,
        depth: false,
        stencil: false,
        desynchronized: false,
        antialias: true,
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

    let context = Rc::new(gl);

    // Settings
    let grid_spacing: u32 = 20 * pixel_ratio;
    let grid_width: u32 = 128;
    let grid_height: u32 = 128;

    let fluid_simulation_fps: f32 = 15.0;
    let viscosity: f32 = 1.2;
    let velocity_dissipation: f32 = 0.2;
    let adjust_advection: f32 = 10.0;

    let max_frame_time: f32 = 1.0 / 10.0;
    let fluid_frame_time: f32 = 1.0 / fluid_simulation_fps;

    let view_scale: f32 = 1.4;
    // TODO: deal with result
    let fluid = Fluid::new(
        &context,
        grid_width,
        grid_height,
        viscosity,
        velocity_dissipation,
    )
    .unwrap();

    let mut noise = Noise::new(&context, grid_width, grid_height).unwrap();
    let drawer = Drawer::new(
        &context,
        width,
        height,
        grid_spacing,
        view_scale,
    )
    .unwrap();

    let mut elapsed_time: f32 = 0.0;
    let mut last_timestamp: f32 = 0.0;
    let mut frame_time: f32 = 0.0;

    noise.generate(elapsed_time);
    noise.blend_noise_into(&fluid.get_velocity_textures(), fluid_frame_time);
    // Finish setup before running the main rendering loop
    context.flush();

    // TODO: clean this up
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    let animate: Box<dyn FnMut(f32)> = Box::new(move |timestamp| {
        context.clear_color(0.0, 0.0, 0.0, 1.0);
        context.clear(GL::COLOR_BUFFER_BIT);

        let timestep = max_frame_time.min(0.001 * (timestamp - last_timestamp));
        last_timestamp = timestamp;
        elapsed_time += timestep;
        frame_time += timestep;

        while frame_time >= fluid_frame_time {
            noise.generate(elapsed_time);

            // Convection
            fluid.advect(fluid_frame_time);

            noise.blend_noise_into(&fluid.get_velocity_textures(), fluid_frame_time);

            fluid.diffuse(fluid_frame_time);

            // TODO: this needs a second pass. See GPU Gems.
            // fluid.curl(fluid_frame_time);

            fluid.calculate_divergence();
            fluid.solve_pressure();
            fluid.subtract_gradient();

            frame_time -= fluid_frame_time;
        }

        // Debugging
        // drawer.draw_texture(&noise.get_noise());
        // drawer.draw_texture(&fluid.get_velocity());
        // drawer.draw_texture(&fluid.get_pressure());

    	// TODO: the line animation is still dependent on the client’s fps. Is
    	// this worth fixing?
        drawer.place_lines(timestep * adjust_advection, &fluid.get_velocity());
        drawer.draw_lines();
        drawer.draw_endpoints();

        web::request_animation_frame(f.borrow().as_ref().unwrap());
    });

    *g.borrow_mut() = Some(Closure::wrap(animate));
    web::request_animation_frame(g.borrow().as_ref().unwrap());

    Ok(())
}
