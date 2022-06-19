use flux::settings::{ClearPressure, ColorScheme, Mode, Noise, Settings};
use flux::Flux;
use glutin::event::{Event, WindowEvent};
use glutin::event_loop::{ControlFlow, EventLoop};
use glutin::window::Window;
use glutin::PossiblyCurrent;
use std::rc::Rc;

fn main() {
    env_logger::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    let settings = Settings {
        mode: Mode::Normal,
        fluid_size: 128,
        fluid_frame_rate: 60.0,
        fluid_timestep: 1.0 / 60.0,
        viscosity: 5.0,
        velocity_dissipation: 0.0,
        clear_pressure: ClearPressure::KeepPressure,
        diffusion_iterations: 3,
        pressure_iterations: 19,
        color_scheme: ColorScheme::Peacock,
        line_length: 700.0,
        line_width: 14.0,
        line_begin_offset: 0.35,
        line_variance: 0.35,
        grid_spacing: 15,
        view_scale: 1.6,
        noise_channels: vec![
            Noise {
                scale: 2.5,
                multiplier: 1.0,
                offset_increment: 0.0015,
            },
            Noise {
                scale: 15.0,
                multiplier: 0.7,
                offset_increment: 0.0015 * 6.0,
            },
            Noise {
                scale: 30.0,
                multiplier: 0.5,
                offset_increment: 0.0015 * 12.0,
            },
        ],
    };

    // let logical_size = glutin::dpi::LogicalSize::new(2560, 480);
    let logical_size = glutin::dpi::LogicalSize::new(1280, 800);
    let (context, window, event_loop) = get_rendering_context(logical_size);
    let physical_size = logical_size.to_physical(window.window().scale_factor());

    let context = Rc::new(context);
    let mut flux = Flux::new(
        &context,
        logical_size.width,
        logical_size.height,
        physical_size.width,
        physical_size.height,
        &Rc::new(settings),
    )
    .unwrap();

    let start = std::time::Instant::now();

    event_loop.run(move |event, _, control_flow| {
        let next_frame_time =
            std::time::Instant::now() + std::time::Duration::from_nanos(16_666_667);
        *control_flow = glutin::event_loop::ControlFlow::WaitUntil(next_frame_time);

        match event {
            Event::LoopDestroyed => {
                return;
            }

            Event::MainEventsCleared => {
                window.window().request_redraw();
            }

            Event::RedrawRequested(_) => {
                flux.animate(start.elapsed().as_millis() as f64);
                window.swap_buffers().unwrap();
            }

            Event::WindowEvent { ref event, .. } => match event {
                WindowEvent::Resized(physical_size) => {
                    window.resize(*physical_size);
                    let logical_size = physical_size.to_logical(window.window().scale_factor());
                    flux.resize(
                        logical_size.width,
                        logical_size.height,
                        physical_size.width,
                        physical_size.height,
                    );
                }
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                _ => (),
            },
            _ => (),
        }
    });
}

pub fn get_rendering_context(
    logical_size: glutin::dpi::LogicalSize<u32>,
) -> (
    glow::Context,
    glutin::ContextWrapper<PossiblyCurrent, Window>,
    EventLoop<()>,
) {
    let event_loop = glutin::event_loop::EventLoop::new();
    let window_builder = glutin::window::WindowBuilder::new()
        .with_title("Flux")
        .with_decorations(true)
        .with_resizable(true)
        .with_inner_size(logical_size);
    let window = glutin::ContextBuilder::new()
        .with_vsync(true)
        .with_multisampling(0)
        .with_double_buffer(Some(true))
        .with_gl_profile(glutin::GlProfile::Core)
        .build_windowed(window_builder, &event_loop)
        .unwrap();
    let window = unsafe { window.make_current().unwrap() };

    let gl =
        unsafe { glow::Context::from_loader_function(|s| window.get_proc_address(s) as *const _) };

    (gl, window, event_loop)
}
