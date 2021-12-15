use crate::data;
use crate::render;
use render::{
    BindingInfo, Buffer, Context, DoubleFramebuffer, Framebuffer, Indices, Program, RenderPass,
    TextureOptions, Uniform, UniformValue, VertexBuffer,
};

use web_sys::WebGl2RenderingContext as GL;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

static FLUID_VERT_SHADER: &'static str = include_str!("./shaders/fluid.vert");
static SIMPLEX_NOISE_FRAG_SHADER: &'static str = include_str!("./shaders/simplex_noise.frag");
static BLEND_NOISE_FRAG_SHADER: &'static str = include_str!("./shaders/blend_noise.frag");

pub struct Noise {
    texture: Framebuffer,
    generate_noise_pass: RenderPass,
    blend_noise_pass: RenderPass,
}

impl Noise {
    pub fn new(context: &Context, width: u32, height: u32) -> Result<Self> {
        let texture_options: TextureOptions = TextureOptions {
            mag_filter: GL::LINEAR,
            min_filter: GL::LINEAR,
            ..Default::default()
        };

        let texture = Framebuffer::new(&context, width, height, texture_options)?
            .with_f32_data(&vec![0.0; (width * height * 4) as usize])?;

        // Geometry
        let plane_vertices = Buffer::from_f32(
            &context,
            &data::PLANE_VERTICES.to_vec(),
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )
        .unwrap();
        let plane_indices = Buffer::from_u16(
            &context,
            &data::PLANE_INDICES.to_vec(),
            GL::ELEMENT_ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )
        .unwrap();

        let simplex_noise_program =
            Program::new(&context, (FLUID_VERT_SHADER, SIMPLEX_NOISE_FRAG_SHADER))?;
        let blend_noise_program =
            Program::new(&context, (FLUID_VERT_SHADER, BLEND_NOISE_FRAG_SHADER))?;

        let generate_noise_pass = RenderPass::new(
            &context,
            vec![VertexBuffer {
                buffer: plane_vertices.clone(),
                binding: BindingInfo {
                    name: "position".to_string(),
                    size: 3,
                    type_: GL::FLOAT,
                    ..Default::default()
                },
            }],
            Indices::IndexBuffer {
                buffer: plane_indices.clone(),
                primitive: GL::TRIANGLES,
            },
            simplex_noise_program,
        )
        .unwrap();

        let blend_noise_pass = RenderPass::new(
            &context,
            vec![VertexBuffer {
                buffer: plane_vertices.clone(),
                binding: BindingInfo {
                    name: "position".to_string(),
                    size: 3,
                    type_: GL::FLOAT,
                    ..Default::default()
                },
            }],
            Indices::IndexBuffer {
                buffer: plane_indices.clone(),
                primitive: GL::TRIANGLES,
            },
            blend_noise_program,
        )
        .unwrap();

        Ok(Self {
            texture,
            generate_noise_pass,
            blend_noise_pass,
        })
    }

    pub fn generate(&mut self, timestep: f32) -> () {
        let width = self.texture.width;
        let height = self.texture.height;

        self.generate_noise_pass
            .draw_to(
                &self.texture,
                &vec![
                    Uniform {
                        name: "uResolution",
                        value: UniformValue::Vec2([width as f32, height as f32]),
                    },
                    Uniform {
                        name: "deltaT",
                        value: UniformValue::Float(timestep),
                    },
                ],
                1,
            )
            .unwrap();
    }

    pub fn blend_noise_into(&mut self, textures: &DoubleFramebuffer, timestep: f32) -> () {
        self.blend_noise_pass
            .draw_to(
                &textures.next(),
                &vec![
                    Uniform {
                        name: "uTexelSize",
                        value: UniformValue::Vec2([
                            1.0 / self.texture.width as f32,
                            1.0 / self.texture.height as f32,
                        ]),
                    },
                    Uniform {
                        name: "deltaT",
                        value: UniformValue::Float(timestep),
                    },
                    Uniform {
                        name: "inputTexture",
                        value: UniformValue::Texture2D(&textures.current().texture, 0),
                    },
                    Uniform {
                        name: "noiseTexture",
                        value: UniformValue::Texture2D(&self.texture.texture, 1),
                    },
                ],
                1,
            )
            .unwrap();

        textures.swap();
    }

    pub fn get_noise(&self) -> &Framebuffer {
        &self.texture
    }
}
