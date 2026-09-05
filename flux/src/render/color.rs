use image::RgbaImage;

pub enum Problem {
    ReadImage(std::io::Error),
    DecodeColorTexture(image::ImageError),
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Problem::ReadImage(err) => write!(f, "Failed to read image: {}", err),
            Problem::DecodeColorTexture(err) => {
                write!(f, "Failed to decode color texture: {}", err)
            }
        }
    }
}

pub struct Context {
    #[allow(dead_code)]
    texture: wgpu::Texture,
    pub texture_view: wgpu::TextureView,
}

impl Context {
    // pub fn load_color_texture_from_file(device: &wgpu::Device, queue: &wgpu::Queue, path: &path::PathBuf) -> Result<Context, Problem> {
    //     std::fs::read(path)
    //         .map_err(Problem::ReadImage)
    //         .and_then(|ref encoded_bytes| Self::load_color_texture(device, queue, encoded_bytes))
    //         .map_err(|err| {
    //             log::error!("Failed to load image from {}: {}", path.display(), err);
    //             err
    //         })
    // }

    pub fn decode_color_texture(encoded_bytes: &[u8]) -> Result<RgbaImage, Problem> {
        log::debug!("Decoding image");

        let mut img =
            image::load_from_memory(encoded_bytes).map_err(Problem::DecodeColorTexture)?;
        if u32::max(img.width(), img.height()) > 640 {
            img = img.resize(640, 400, image::imageops::FilterType::Nearest);
        }

        log::debug!(
            "Uploading image (width: {}, height: {})",
            img.width(),
            img.height()
        );

        Ok(img.to_rgba8())
    }
}

pub fn load_color_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    img: &RgbaImage,
) -> wgpu::TextureView {
    let width = img.width();
    let height = img.height();
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    // Create a buffer to store the image data
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        view_formats: &[],
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
    });

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        // TODO: fix clone()
        &img.clone().into_raw(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: None,
        },
        size,
    );

    texture.create_view(&wgpu::TextureViewDescriptor::default())
}
