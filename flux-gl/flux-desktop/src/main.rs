use std::num::NonZeroU32;
use std::rc::Rc;

use flux::settings::{ColorMode, Settings};
use flux::Flux;

use glutin::config::ConfigTemplateBuilder;
use glutin::context::{ContextAttributesBuilder, GlProfile, NotCurrentGlContext};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::prelude::*;
use glutin::surface::{SurfaceAttributesBuilder, SwapInterval, WindowSurface};

use glutin_winit::DisplayBuilder;

use raw_window_handle::HasWindowHandle;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesExtMacOS;

struct GlState {
    surface: glutin::surface::Surface<WindowSurface>,
    context: glutin::context::PossiblyCurrentContext,
}

struct App {
    window: Option<Window>,
    gl_state: Option<GlState>,
    gl_context: Option<Rc<glow::Context>>,
    flux: Option<Flux>,
    start: std::time::Instant,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

    let mut app = App {
        window: None,
        gl_state: None,
        gl_context: None,
        flux: None,
        start: std::time::Instant::now(),
    };

    event_loop.run_app(&mut app).unwrap();
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let logical_size = LogicalSize::new(1280, 800);

        #[cfg(target_os = "macos")]
        let window_attributes = Window::default_attributes()
            .with_title("Flux")
            .with_inner_size(logical_size)
            .with_resizable(true)
            .with_title_hidden(true)
            .with_titlebar_transparent(true)
            .with_fullsize_content_view(true);

        #[cfg(not(target_os = "macos"))]
        let window_attributes = Window::default_attributes()
            .with_title("Flux")
            .with_decorations(true)
            .with_resizable(true)
            .with_inner_size(logical_size);

        let template = ConfigTemplateBuilder::new();

        let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attributes));

        let (window, gl_config) = display_builder
            .build(event_loop, template, |configs| {
                configs
                    .reduce(|accum, config| {
                        if config.num_samples() > accum.num_samples() {
                            config
                        } else {
                            accum
                        }
                    })
                    .unwrap()
            })
            .unwrap();

        let window = window.unwrap();
        let raw_window_handle = window.window_handle().unwrap().as_raw();

        let gl_display = gl_config.display();

        let context_attributes = ContextAttributesBuilder::new()
            .with_profile(GlProfile::Core)
            .build(Some(raw_window_handle));

        let not_current_context = unsafe {
            gl_display
                .create_context(&gl_config, &context_attributes)
                .unwrap()
        };

        let physical_size = window.inner_size();
        let (width, height) = (
            NonZeroU32::new(physical_size.width.max(1)).unwrap(),
            NonZeroU32::new(physical_size.height.max(1)).unwrap(),
        );

        let surface_attributes = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            raw_window_handle,
            width,
            height,
        );

        let surface = unsafe {
            gl_display
                .create_window_surface(&gl_config, &surface_attributes)
                .unwrap()
        };

        let context = not_current_context.make_current(&surface).unwrap();

        let _ =
            surface.set_swap_interval(&context, SwapInterval::Wait(NonZeroU32::new(1).unwrap()));

        let gl =
            unsafe { glow::Context::from_loader_function_cstr(|s| gl_display.get_proc_address(s)) };
        let gl = Rc::new(gl);

        let logical_size: LogicalSize<u32> = physical_size.to_logical(window.scale_factor());

        let flux = Flux::new(
            &gl,
            logical_size.width,
            logical_size.height,
            physical_size.width,
            physical_size.height,
            &Rc::new(Settings {
                seed: Some("1337".into()),
                ..Default::default()
            }),
        )
        .unwrap();

        self.start = std::time::Instant::now();
        self.window = Some(window);
        self.gl_state = Some(GlState { surface, context });
        self.gl_context = Some(gl);
        self.flux = Some(flux);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let (Some(window), Some(gl_state), Some(flux)) = (
            self.window.as_ref(),
            self.gl_state.as_ref(),
            self.flux.as_mut(),
        ) else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::DroppedFile(path) => {
                let settings = Settings {
                    color_mode: ColorMode::ImageFile(path.into()),
                    ..Default::default()
                };
                flux.update(&Rc::new(settings));
            }

            WindowEvent::Resized(physical_size) => {
                let (width, height) = (
                    NonZeroU32::new(physical_size.width.max(1)).unwrap(),
                    NonZeroU32::new(physical_size.height.max(1)).unwrap(),
                );
                gl_state.surface.resize(&gl_state.context, width, height);

                let logical_size: LogicalSize<u32> =
                    physical_size.to_logical(window.scale_factor());
                flux.resize(
                    logical_size.width,
                    logical_size.height,
                    physical_size.width,
                    physical_size.height,
                );
            }

            WindowEvent::RedrawRequested => {
                flux.animate(self.start.elapsed().as_secs_f64() * 1000.0);
                gl_state.surface.swap_buffers(&gl_state.context).unwrap();
                window.request_redraw();
            }

            _ => (),
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}
