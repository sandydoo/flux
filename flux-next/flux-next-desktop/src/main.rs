// Disable the console window that pops up when you launch the .exe
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

#[cfg(target_os = "macos")]
use winit::platform::macos::WindowBuilderExtMacOS;

use flux_next::{Flux, Settings};

fn main() -> Result<(), impl std::error::Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error")).init();

    let event_loop = EventLoop::new().unwrap();
    let logical_size = winit::dpi::LogicalSize::new(1280, 800);

    #[cfg(target_os = "macos")]
    let window = WindowBuilder::new()
        .with_title("Flux")
        .with_decorations(true)
        .with_resizable(true)
        .with_inner_size(logical_size)
        .with_title_hidden(true)
        .with_titlebar_transparent(true)
        .with_fullsize_content_view(true)
        .build(&event_loop)
        .unwrap();

    #[cfg(not(target_os = "macos"))]
    let window = WindowBuilder::new()
        .with_title("Flux")
        .with_decorations(true)
        .with_resizable(true)
        .with_inner_size(logical_size)
        .build(&event_loop)
        .unwrap();

    pollster::block_on(run(event_loop, window))
}

async fn run(
    event_loop: EventLoop<()>,
    window: winit::window::Window,
) -> Result<(), impl std::error::Error> {
    let wgpu_instance = wgpu::Instance::default();
    let window_surface = unsafe { wgpu_instance.create_surface(&window) }.unwrap();
    let adapter = wgpu_instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&window_surface),
        })
        .await
        .expect("Failed to find an appropiate adapter");
    print!(
        "{:?}\n{:?}",
        adapter.features(),
        adapter.limits().max_push_constant_size
    );

    // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
    let mut limits = wgpu::Limits::default().using_resolution(adapter.limits());
    // Request push constants for the shaders
    let required_push_constant_size = 8;
    limits.max_push_constant_size = required_push_constant_size;
    let features =
        wgpu::Features::PUSH_CONSTANTS | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;

    let (device, command_queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features,
                limits,
            },
            None,
        )
        .await
        .expect("Failed to create device");

    let swapchain_capabilities = window_surface.get_capabilities(&adapter);
    let swapchain_format = get_preferred_format(&swapchain_capabilities);
    log::debug!("Swapchain format: {:?}", swapchain_format);

    let physical_size = window.inner_size();
    let mut config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: swapchain_format,
        width: physical_size.width,
        height: physical_size.height,
        present_mode: wgpu::PresentMode::AutoVsync,
        alpha_mode: swapchain_capabilities.alpha_modes[0],
        view_formats: vec![],
    };

    window_surface.configure(&device, &config);

    let logical_size = window.inner_size();
    let mut flux = Flux::new(
        &device,
        &command_queue,
        swapchain_format,
        logical_size.width,
        logical_size.height,
        physical_size.width,
        physical_size.height,
        &Arc::new(Settings::default()),
    )
    .unwrap();

    let start = std::time::Instant::now();

    event_loop.run(move |event, elwt| {
        // Have the closure take ownership of the resources.
        // `event_loop.run` never returns, therefore we must do this to ensure
        // the resources are properly cleaned up.
        let _ = (&wgpu_instance, &adapter, &flux);

        elwt.set_control_flow(winit::event_loop::ControlFlow::Poll);

        match event {
            Event::AboutToWait => {
                window.request_redraw();
            }
            Event::WindowEvent { event, window_id } if window_id == window.id() => match event {
                WindowEvent::CloseRequested
                | WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            physical_key: PhysicalKey::Code(KeyCode::Escape),
                            state: ElementState::Released,
                            ..
                        },
                    ..
                } => elwt.exit(),
                WindowEvent::DroppedFile(path) => {
                    let bytes = std::fs::read(&path).unwrap();
                    flux.sample_colors_from_image(bytes);
                    window.request_redraw();
                }
                WindowEvent::Resized(new_size) => {
                    config.width = new_size.width.max(1);
                    config.height = new_size.height.max(1);
                    window_surface.configure(&device, &config);

                    let logical_size = new_size.to_logical(window.scale_factor());
                    flux.resize(
                        logical_size.width,
                        logical_size.height,
                        physical_size.width,
                        physical_size.height,
                    );
                    window.request_redraw();
                }
                WindowEvent::RedrawRequested => {
                    let frame = window_surface
                        .get_current_texture()
                        .expect("Failed to acquire next swap chain texture");
                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    let mut encoder =
                        device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("flux:render"),
                        });

                    flux.animate(
                        &device,
                        &command_queue,
                        &mut encoder,
                        &view,
                        start.elapsed().as_secs_f64() * 1000.0,
                    );

                    command_queue.submit(Some(encoder.finish()));
                    frame.present();
                }
                _ => (),
            },
            _ => (),
        }
    })
}

fn get_preferred_format(capabilities: &wgpu::SurfaceCapabilities) -> wgpu::TextureFormat {
    // Prefer non-srgb formats, as we will be doing linear math in the shaders.
    // If the swapchain doesn't support any non-srgb formats, we will fall back to srgb.
    let preferred_formats = [
        wgpu::TextureFormat::Rgb10a2Unorm, // TODO: does 10-bit make a difference here?
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ];

    for format in &preferred_formats {
        if capabilities.formats.contains(format) {
            return *format;
        }
    }

    // If none of the preferred formats are supported, just return the first supported format.
    capabilities.formats[0]
}
