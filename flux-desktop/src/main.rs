// Disable the console window that pops up when you launch the .exe
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use image::RgbaImage;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesExtMacOS;

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;

    type CGColorSpaceRef = *const c_void;
    type CFStringRef = *const c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        static kCGColorSpaceExtendedSRGB: CFStringRef;
        fn CGColorSpaceCreateWithName(name: CFStringRef) -> CGColorSpaceRef;
        fn CFRelease(cf: *const c_void);
    }

    extern "C" {
        fn sel_registerName(name: *const u8) -> *const c_void;
    }

    // objc_msgSend is NOT variadic — it uses the callee's calling convention.
    // On ARM64, variadic and non-variadic conventions differ (stack vs registers),
    // so we must call it with the correct function pointer type for each message.
    #[allow(clashing_extern_declarations)]
    extern "C" {
        #[link_name = "objc_msgSend"]
        fn objc_msg_send_bool(receiver: *mut c_void, sel: *const c_void, val: i32);
        #[link_name = "objc_msgSend"]
        fn objc_msg_send_ptr(receiver: *mut c_void, sel: *const c_void, val: *const c_void);
    }

    /// Set the CAMetalLayer's color space to extended linear sRGB and enable EDR.
    ///
    /// Uses wgpu's HAL API to access the Metal surface's layer directly.
    pub fn enable_edr(surface: &wgpu::Surface) {
        unsafe {
            let hal_surface = surface.as_hal::<wgpu::hal::api::Metal>();
            let Some(hal_surface) = hal_surface else {
                log::warn!("Failed to access Metal HAL surface");
                return;
            };

            let layer = hal_surface.render_layer().lock();
            let layer_ptr = metal::foreign_types::ForeignType::as_ptr(&*layer) as *mut c_void;

            // Enable extended dynamic range content
            objc_msg_send_bool(
                layer_ptr,
                sel_registerName(b"setWantsExtendedDynamicRangeContent:\0".as_ptr()),
                1,
            );

            // Set the color space to extended sRGB (non-linear).
            // This preserves the sRGB gamma interpretation of our pixel values
            // (matching the default layer behavior) while allowing values > 1.0
            // to map to EDR brightness above SDR white.
            let color_space = CGColorSpaceCreateWithName(kCGColorSpaceExtendedSRGB);
            if !color_space.is_null() {
                objc_msg_send_ptr(
                    layer_ptr,
                    sel_registerName(b"setColorspace:\0".as_ptr()),
                    color_space,
                );
                CFRelease(color_space);
                log::info!("Enabled EDR with extended sRGB color space");
            } else {
                log::warn!("Failed to create extended sRGB color space");
            }
        }
    }
}

use flux::{Flux, Settings};

struct App {
    runtime: tokio::runtime::Runtime,
    tx: mpsc::Sender<Msg>,
    rx: mpsc::Receiver<Msg>,

    flux: Flux,
    _settings: Arc<Settings>,

    color_image: Arc<Mutex<Option<RgbaImage>>>,
}

enum Msg {
    DecodedImage,
}

impl App {
    fn handle_pending_messages(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::DecodedImage => {
                    if let Some(image) = &*self.color_image.lock().unwrap() {
                        self.flux.sample_colors_from_image(device, queue, image);
                    }
                }
            }
        }
    }

    pub fn decode_image(&self, encoded_bytes: Vec<u8>) {
        let tx = self.tx.clone();
        let color_image = Arc::clone(&self.color_image);
        self.runtime.spawn(async move {
            match flux::render::color::Context::decode_color_texture(&encoded_bytes) {
                Ok(image) => {
                    {
                        let mut boop = color_image.lock().unwrap();
                        *boop = Some(image);
                    }
                    if tx.send(Msg::DecodedImage).await.is_err() {
                        log::error!("Failed to send decoded image message");
                    }
                }
                Err(err) => log::error!("{}", err),
            }
        });
        log::debug!("Spawned image decoding task");
    }
}

struct GpuState {
    device: wgpu::Device,
    command_queue: wgpu::Queue,
    window_surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

struct FluxApp {
    runtime: tokio::runtime::Runtime,
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
    app: Option<App>,
    start: std::time::Instant,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

    let mut flux_app = FluxApp {
        runtime,
        window: None,
        gpu: None,
        app: None,
        start: std::time::Instant::now(),
    };

    event_loop.run_app(&mut flux_app)?;
    Ok(())
}

impl ApplicationHandler for FluxApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let logical_size = winit::dpi::LogicalSize::new(1280, 800);

        #[cfg(target_os = "macos")]
        let window_attributes = Window::default_attributes()
            .with_title("Flux")
            .with_decorations(true)
            .with_resizable(true)
            .with_inner_size(logical_size)
            .with_title_hidden(true)
            .with_titlebar_transparent(true)
            .with_fullsize_content_view(true);

        #[cfg(not(target_os = "macos"))]
        let window_attributes = Window::default_attributes()
            .with_title("Flux")
            .with_decorations(true)
            .with_resizable(true)
            .with_inner_size(logical_size);

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        let wgpu_instance = wgpu::Instance::default();
        let window_surface = wgpu_instance.create_surface(window.clone()).unwrap();
        let adapter =
            pollster::block_on(wgpu_instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&window_surface),
            }))
            .expect("Failed to find an appropriate adapter");

        let limits = wgpu::Limits::default().using_resolution(adapter.limits());
        let features = wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
            | wgpu::Features::FLOAT32_FILTERABLE;

        let (device, command_queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: features,
                required_limits: limits,
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            }))
            .expect("Failed to create device");

        let swapchain_capabilities = window_surface.get_capabilities(&adapter);
        let swapchain_format = get_preferred_format(&swapchain_capabilities);
        log::info!("Swapchain format: {:?}", swapchain_format);
        log::info!("Available formats: {:?}", swapchain_capabilities.formats);

        let physical_size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: physical_size.width,
            height: physical_size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![],
        };

        window_surface.configure(&device, &config);

        #[cfg(target_os = "macos")]
        macos::enable_edr(&window_surface);

        let logical_size = physical_size.to_logical(window.scale_factor());
        let settings = Arc::new(Settings {
            seed: Some("1337".into()),
            ..Default::default()
        });
        let flux = Flux::new(
            &device,
            &command_queue,
            swapchain_format,
            logical_size.width,
            logical_size.height,
            physical_size.width,
            physical_size.height,
            &Arc::clone(&settings),
        )
        .unwrap();

        window.set_visible(true);

        let (tx, rx) = mpsc::channel(32);

        // Take the runtime out temporarily to create the App
        let runtime = std::mem::replace(
            &mut self.runtime,
            tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap(),
        );

        self.app = Some(App {
            runtime,
            tx,
            rx,
            flux,
            _settings: settings,
            color_image: Arc::new(Mutex::new(None)),
        });

        self.gpu = Some(GpuState {
            device,
            command_queue,
            window_surface,
            config,
        });

        self.window = Some(window);
        self.start = std::time::Instant::now();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let (Some(window), Some(gpu), Some(app)) =
            (self.window.as_ref(), self.gpu.as_mut(), self.app.as_mut())
        else {
            return;
        };

        if window_id != window.id() {
            return;
        }

        app.handle_pending_messages(&gpu.device, &gpu.command_queue);

        match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        state: ElementState::Released,
                        ..
                    },
                ..
            } => event_loop.exit(),
            WindowEvent::DroppedFile(path) => {
                let bytes = std::fs::read(path).unwrap();
                app.decode_image(bytes);
                window.request_redraw();
            }
            WindowEvent::Resized(new_size) => {
                gpu.config.width = new_size.width.max(1);
                gpu.config.height = new_size.height.max(1);
                gpu.window_surface.configure(&gpu.device, &gpu.config);

                let physical_size = window.inner_size();
                let logical_size = new_size.to_logical(window.scale_factor());
                app.flux.resize(
                    &gpu.device,
                    &gpu.command_queue,
                    logical_size.width,
                    logical_size.height,
                    physical_size.width,
                    physical_size.height,
                );
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let frame = gpu
                    .window_surface
                    .get_current_texture()
                    .expect("Failed to acquire next swap chain texture");
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder =
                    gpu.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("flux:render"),
                        });

                app.flux.animate(
                    &gpu.device,
                    &gpu.command_queue,
                    &mut encoder,
                    &view,
                    None,
                    self.start.elapsed().as_secs_f64() * 1000.0,
                );

                gpu.command_queue.submit(Some(encoder.finish()));
                window.pre_present_notify();
                frame.present();
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

fn get_preferred_format(capabilities: &wgpu::SurfaceCapabilities) -> wgpu::TextureFormat {
    // Prefer non-srgb formats, as we will be doing linear math in the shaders.
    // If the swapchain doesn't support any non-srgb formats, we will fall back to srgb.
    //
    // On macOS, prefer Rgba16Float for EDR support: values > 1.0 in the framebuffer
    // map to brightness above SDR white when the CAMetalLayer is configured with
    // an extended linear sRGB color space.
    let preferred_formats = [
        #[cfg(target_os = "macos")]
        wgpu::TextureFormat::Rgba16Float,
        wgpu::TextureFormat::Rgb10a2Unorm,
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
