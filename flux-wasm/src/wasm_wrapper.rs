use flux::{self, settings};
use gloo_utils::format::JsValueSerdeExt;
use serde::Serialize;
use std::rc::Rc;
use std::sync::Arc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::Window;
use winit::event_loop::EventLoop;
use winit::platform::web::WindowBuilderExtWebSys;

#[wasm_bindgen]
pub struct Flux {
    canvas: Canvas,
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<winit::window::Window>,
    window_surface: wgpu::Surface<'static>,
    logical_width: u32,
    logical_height: u32,
    pixel_ratio: f64,
    instance: flux::Flux,
}

#[wasm_bindgen]
impl Flux {
    #[wasm_bindgen(setter)]
    pub fn set_settings(&mut self, settings_object: &JsValue) {
        let settings: settings::Settings = settings_object.into_serde().unwrap();
        self.instance
            .update(&self.device, &self.queue, &Arc::new(settings));
    }

    #[wasm_bindgen]
    pub fn save_image(&mut self, bitmap: web_sys::ImageBitmap) {
        let width = bitmap.width();
        let height = bitmap.height();
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let source = wgpu::ImageCopyExternalImage {
            source: wgpu::ExternalImageSource::ImageBitmap(bitmap),
            origin: wgpu::Origin2d::ZERO,
            flip_y: false,
        };

        // Create a buffer to store the image data
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            view_formats: &[],
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
        });

        let dest = wgpu::ImageCopyTextureTagged {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
            color_space: wgpu::PredefinedColorSpace::Srgb,
            premultiplied_alpha: false,
        };

        self.queue
            .copy_external_image_to_texture(&source, dest, size);

        let texture = dest.texture;
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.instance
            .sample_colors_from_texture_view(&self.device, &self.queue, texture_view);
    }

    #[wasm_bindgen(constructor)]
    pub async fn new(settings_object: &JsValue) -> Result<Flux, JsValue> {
        console_log::init_with_level(log::Level::Trace).expect("cannot enable logging");

        set_panic_hook();

        let event_loop = EventLoop::new().unwrap();

        let element_id = "canvas";
        let window = web_sys::window().expect("no global `window` exists");
        let document = window.document().expect("should have a document on window");
        let html_canvas = document
            .get_element_by_id(element_id)
            .map(|element| element.dyn_into::<web_sys::HtmlCanvasElement>())
            .unwrap_or_else(|| {
                panic!(
                    "I expected to find a canvas element with id `{}`",
                    element_id
                )
            })?;

        let pixel_ratio: f64 = window.device_pixel_ratio();
        let logical_width = html_canvas.client_width() as u32;
        let logical_height = html_canvas.client_height() as u32;
        let (physical_width, physical_height) =
            physical_from_logical_size(logical_width, logical_height, pixel_ratio);
        html_canvas.set_width(physical_width);
        html_canvas.set_height(physical_height);

        let window = winit::window::WindowBuilder::new()
            .with_canvas(Some(html_canvas.clone()))
            .build(&event_loop)
            .unwrap();

        let canvas = Canvas::new(html_canvas);

        let settings = match settings_object.into_serde() {
            Ok(settings) => Arc::new(settings),
            Err(msg) => return Err(JsValue::from_str(&msg.to_string())),
        };

        let window = Arc::new(window);
        let mut instance_desc = wgpu::InstanceDescriptor::default();
        instance_desc.backends = wgpu::Backends::BROWSER_WEBGPU;
        let wgpu_instance = wgpu::Instance::new(instance_desc);
        let window_surface = wgpu_instance
            .create_surface(Arc::clone(&window))
            .expect("Failed to create surface");
        let adapter = wgpu_instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&window_surface),
            })
            .await
            .expect("Failed to find an appropiate adapter");

        log::debug!("{:?}\n{:?}", adapter.get_info(), adapter.features(),);

        // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
        let mut limits = wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits());

        let features = wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
            | wgpu::Features::FLOAT32_FILTERABLE;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: features,
                    required_limits: limits,
                },
                None,
            )
            .await
            .expect("Failed to create device");

        let swapchain_capabilities = window_surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];
        log::debug!("Swapchain format: {:?}", swapchain_format);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: physical_width,
            height: physical_height,
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![],
        };

        window_surface.configure(&device, &config);

        let flux = flux::Flux::new(
            &device,
            &queue,
            swapchain_format,
            logical_width,
            logical_height,
            physical_width,
            physical_height,
            &settings,
        )
        .map_err(|err| JsValue::from_str(&err.to_string()))?;

        Ok(Self {
            instance: flux,
            canvas,
            device,
            queue,
            window,
            window_surface,
            logical_width,
            logical_height,
            pixel_ratio,
        })
    }

    pub fn animate(&mut self, timestamp: f64) {
        let frame = self
            .window_surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("flux:render"),
            });

        self.instance
            .animate(&self.device, &self.queue, &mut encoder, &view, timestamp);

        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }

    pub fn resize(&mut self, logical_width: u32, logical_height: u32) {
        if (self.logical_width != logical_width) || (self.logical_height != logical_height) {
            let (physical_width, physical_height) =
                physical_from_logical_size(logical_width, logical_height, self.pixel_ratio);

            self.canvas.set_width(physical_width);
            self.canvas.set_height(physical_height);

            self.instance.resize(
                &self.device,
                &self.queue,
                logical_width,
                logical_height,
                physical_width,
                physical_height,
            );

            self.logical_width = logical_width;
            self.logical_height = logical_height;
        }
    }
}

// An offscreen canvas decouples our canvas from the DOM. Not having to sync
// with the DOM greatly improves performance, but browser support is poor.
// AFAIK, Chrome is the only browser that has implemented the feature.
pub enum Canvas {
    OnscreenCanvas(web_sys::HtmlCanvasElement),
    OffscreenCanvas(web_sys::HtmlCanvasElement, web_sys::OffscreenCanvas),
}

impl Canvas {
    pub fn new(html_canvas: web_sys::HtmlCanvasElement) -> Self {
        // WARN: Panics in Safari.
        // match html_canvas.transfer_control_to_offscreen() {
        //     Ok(offscreen_canvas) => Canvas::OffscreenCanvas(html_canvas, offscreen_canvas),
        //     Err(_) => Canvas::OnscreenCanvas(html_canvas),
        // }

        Canvas::OnscreenCanvas(html_canvas)
    }

    pub fn get_context(&self, context_id: &str) -> Result<Option<js_sys::Object>, JsValue> {
        match self {
            Canvas::OnscreenCanvas(ref canvas) => canvas.get_context(context_id),
            Canvas::OffscreenCanvas(_, ref canvas) => canvas.get_context(context_id),
        }
    }

    pub fn get_context_with_context_options(
        &self,
        context_id: &str,
        context_options: &JsValue,
    ) -> Result<Option<js_sys::Object>, JsValue> {
        match self {
            Canvas::OnscreenCanvas(ref canvas) => {
                canvas.get_context_with_context_options(context_id, context_options)
            }
            Canvas::OffscreenCanvas(_, ref canvas) => {
                canvas.get_context_with_context_options(context_id, context_options)
            }
        }
    }

    pub fn set_width(&self, width: u32) {
        match self {
            Canvas::OnscreenCanvas(ref canvas) => canvas.set_width(width),
            Canvas::OffscreenCanvas(_, ref canvas) => canvas.set_width(width),
        }
    }

    pub fn set_height(&self, height: u32) {
        match self {
            Canvas::OnscreenCanvas(ref canvas) => canvas.set_height(height),
            Canvas::OffscreenCanvas(_, ref canvas) => canvas.set_height(height),
        }
    }
}

pub fn window() -> Window {
    web_sys::window().expect("The global `window` doesn’t exist")
}

// https://github.com/rustwasm/console_error_panic_hook#readme
pub fn set_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

fn physical_from_logical_size(
    logical_width: u32,
    logical_height: u32,
    pixel_ratio: f64,
) -> (u32, u32) {
    (
        (pixel_ratio * f64::from(logical_width)) as u32,
        (pixel_ratio * f64::from(logical_height)) as u32,
    )
}
