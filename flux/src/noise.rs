use crate::{data, render, settings};
use render::{
    BindingInfo, Buffer, Context, DoubleFramebuffer, Framebuffer, Indices, Program, RenderPass,
    TextureOptions, Uniform, UniformValue, VertexBuffer,
};
use settings::Noise;

use web_sys::WebGl2RenderingContext as GL;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

static FLUID_VERT_SHADER: &'static str = include_str!("./shaders/fluid.vert");
static SIMPLEX_NOISE_FRAG_SHADER: &'static str = include_str!("./shaders/simplex_noise.frag");
static BLEND_NOISE_FRAG_SHADER: &'static str = include_str!("./shaders/blend_noise.frag");

pub struct NoiseInjector {
    noise: Noise,

    blend_begin_time: f32,
    last_blend_progress: f32,
    offset1: f32,
    offset2: f32,

    texture: Framebuffer,
    generate_noise_pass: RenderPass,
    blend_noise_pass: RenderPass,
}

impl NoiseInjector {
    pub fn update_noise(&mut self, new_noise: Noise) -> () {
        self.noise = new_noise.clone();
    }

    pub fn new(context: &Context, width: u32, height: u32, noise: Noise) -> Result<Self> {
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
            noise: noise.clone(),

            blend_begin_time: 0.0,
            last_blend_progress: 0.0,
            offset1: noise.offset_1,
            offset2: noise.offset_2,

            texture,
            generate_noise_pass,
            blend_noise_pass,
        })
    }

    pub fn generate_now(&mut self, elapsed_time: f32) -> () {
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
                        name: "uOffset1",
                        value: UniformValue::Float(self.offset1),
                    },
                    Uniform {
                        name: "uOffset2",
                        value: UniformValue::Float(self.offset2),
                    },
                    Uniform {
                        name: "uOffsetIncrement",
                        value: UniformValue::Float(self.noise.offset_increment),
                    },
                    Uniform {
                        name: "uFrequency",
                        value: UniformValue::Float(self.noise.scale),
                    },
                ],
                1,
            )
            .unwrap();

        self.blend_begin_time = elapsed_time;
        self.last_blend_progress = 0.0;
        self.offset1 += self.noise.offset_increment;
        self.offset2 += self.noise.offset_increment;
    }

    pub fn generate(&mut self, elapsed_time: f32) -> () {
        let time_since_last_update = elapsed_time - self.blend_begin_time;

        if time_since_last_update >= self.noise.delay {
            self.generate_now(elapsed_time);
        }
    }

    pub fn blend_noise_into(&mut self, textures: &DoubleFramebuffer, elapsed_time: f32) -> () {
        let blend_progress: f32 =
            ((elapsed_time - self.blend_begin_time) / self.noise.blend_duration).clamp(0.0, 1.0);

        let delta_blend_progress = blend_progress - self.last_blend_progress;

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
                        name: "uMultiplier",
                        value: UniformValue::Float(self.noise.multiplier),
                    },
                    Uniform {
                        name: "uBlendProgress",
                        value: UniformValue::Float(delta_blend_progress),
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
        self.last_blend_progress = blend_progress;
    }

    pub fn get_noise(&self) -> &Framebuffer {
        &self.texture
    }
}
