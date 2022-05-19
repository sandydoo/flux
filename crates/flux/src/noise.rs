use crate::{data, render, settings};
use render::{
    Buffer, Context, DoubleFramebuffer, Framebuffer, Program, TextureOptions, Uniform,
    UniformValue, VertexArrayObject, VertexBufferLayout,
};

use crevice::std140::{self, AsStd140};
use glow::HasContext;
use half::f16;
use std::rc::Rc;

static NOISE_VERT_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/noise.vert"));
static GENERATE_NOISE_FRAG_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/generate_noise.frag"));
static INJECT_NOISE_FRAG_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/inject_noise.frag"));

#[derive(AsStd140)]
pub struct NoiseUniforms {
    scale: f32,
    offset_1: f32,
    offset_2: f32,
    multiplier: f32,
    blend_factor: f32,
}

pub struct NoiseChannel {
    settings: settings::Noise,
    offset_1: f32,
    offset_2: f32,
    blend_begin_time: f32,
    last_blend_progress: f32,
}

impl NoiseChannel {
    pub fn tick(&mut self, elapsed_time: f32) -> () {
        self.blend_begin_time = elapsed_time;
        self.last_blend_progress = 0.0;
        self.offset_1 += self.settings.offset_increment;
    }
}

pub struct NoiseGenerator {
    context: Context,
    channels: Vec<NoiseChannel>,
    texture: Framebuffer,

    generate_noise_pass: Program,
    inject_noise_pass: Program,

    noise_buffer: VertexArrayObject,
    uniforms: Buffer,
    #[allow(unused)]
    plane_vertices: Buffer,
    #[allow(unused)]
    plane_indices: Buffer,
}

impl NoiseGenerator {
    pub fn new(context: &Context, width: u32, height: u32) -> NoiseGeneratorBuilder {
        NoiseGeneratorBuilder::new(context, width, height)
    }

    pub fn update(&mut self, new_settings: &[settings::Noise]) -> () {
        for (channel, new_setting) in self.channels.iter_mut().zip(new_settings.iter()) {
            channel.settings = new_setting.clone();
        }
    }

    pub fn generate(&mut self, elapsed_time: f32) -> () {
        let uniforms = &build_noise_uniforms(&self.channels);
        self.uniforms.update(&uniforms);

        self.generate_noise_pass.use_program();

        unsafe {
            self.context.bind_vertex_array(Some(self.noise_buffer.id));

            self.context
                .bind_buffer_base(glow::UNIFORM_BUFFER, 0, Some(self.uniforms.id));

            self.texture.draw_to(&self.context, || {
                self.context
                    .draw_elements(glow::TRIANGLES, 6, glow::UNSIGNED_SHORT, 0);
            });
        }

        for channel in self.channels.iter_mut() {
            channel.tick(elapsed_time);
        }
    }

    pub fn blend_noise_into(&mut self, target_textures: &DoubleFramebuffer, timestep: f32) -> () {
        target_textures.draw_to(&self.context, |target_texture| {
            self.inject_noise_pass.use_program();

            unsafe {
                self.context.disable(glow::BLEND);
                self.context.bind_vertex_array(Some(self.noise_buffer.id));

                self.inject_noise_pass.set_uniform(&Uniform {
                    name: "deltaTime",
                    value: UniformValue::Float(timestep),
                });

                self.context.active_texture(glow::TEXTURE0);
                self.context
                    .bind_texture(glow::TEXTURE_2D, Some(target_texture.texture));

                self.context.active_texture(glow::TEXTURE1);
                self.context
                    .bind_texture(glow::TEXTURE_2D, Some(self.texture.texture));

                self.context
                    .draw_elements(glow::TRIANGLES, 6, glow::UNSIGNED_SHORT, 0);
            }
        });
    }

    #[allow(dead_code)]
    pub fn get_noise(&self) -> &Framebuffer {
        &self.texture
    }
}

pub struct NoiseGeneratorBuilder {
    context: Context,
    width: u32,
    height: u32,
    channels: Vec<NoiseChannel>,
}

impl NoiseGeneratorBuilder {
    pub fn new(context: &Context, width: u32, height: u32) -> Self {
        NoiseGeneratorBuilder {
            context: Rc::clone(context),
            width,
            height,
            channels: Vec::new(),
        }
    }

    pub fn add_channel(&mut self, channel: &settings::Noise) -> &Self {
        self.channels.push(NoiseChannel {
            settings: channel.clone(),
            offset_1: 0.0,
            offset_2: 0.0,
            blend_begin_time: 0.0,
            last_blend_progress: 0.0,
        });

        self
    }

    pub fn build(self) -> Result<NoiseGenerator, render::Problem> {
        // Geometry
        let plane_vertices = Buffer::from_f32(
            &self.context,
            &data::PLANE_VERTICES,
            glow::ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;
        let plane_indices = Buffer::from_u16(
            &self.context,
            &data::PLANE_INDICES,
            glow::ELEMENT_ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;
        let texture = Framebuffer::new(
            &self.context,
            self.width,
            self.height,
            TextureOptions {
                mag_filter: glow::LINEAR,
                min_filter: glow::LINEAR,
                format: glow::RG16F,
                ..Default::default()
            },
        )?;
        texture.with_data(None::<&[f16]>)?;

        let generate_noise_pass = Program::new_with_variables(
            &self.context,
            (NOISE_VERT_SHADER, GENERATE_NOISE_FRAG_SHADER),
            &[("CHANNEL_COUNT", &self.channels.len().to_string())],
        )?;
        let inject_noise_pass =
            Program::new(&self.context, (NOISE_VERT_SHADER, INJECT_NOISE_FRAG_SHADER))?;

        let noise_buffer = VertexArrayObject::new(
            &self.context,
            &generate_noise_pass,
            &[(
                &plane_vertices,
                VertexBufferLayout {
                    name: "position",
                    size: 3,
                    type_: glow::FLOAT,
                    ..Default::default()
                },
            )],
            Some(&plane_indices),
        )?;

        let uniforms = Buffer::from_bytes(
            &self.context,
            &build_noise_uniforms(&self.channels),
            glow::ARRAY_BUFFER,
            glow::DYNAMIC_DRAW,
        )?;

        generate_noise_pass.set_uniform_block("Channels", 0);
        inject_noise_pass.set_uniforms(&[
            &Uniform {
                name: "velocityTexture",
                value: UniformValue::Texture2D(0),
            },
            &Uniform {
                name: "noiseTexture",
                value: UniformValue::Texture2D(1),
            },
        ]);

        Ok(NoiseGenerator {
            context: self.context,
            channels: self.channels,
            texture,

            generate_noise_pass,
            inject_noise_pass,

            noise_buffer,
            uniforms,
            plane_vertices,
            plane_indices,
        })
    }
}

fn build_noise_uniforms(channels: &[NoiseChannel]) -> Vec<u8> {
    let noise_uniforms: Vec<NoiseUniforms> = channels
        .iter()
        .map(|channel| NoiseUniforms {
            scale: channel.settings.scale,
            offset_1: channel.offset_1,
            offset_2: channel.offset_2,
            multiplier: channel.settings.multiplier,
            blend_factor: 0.0,
        })
        .collect();
    let mut aligned_uniforms = Vec::new();
    let mut writer = std140::Writer::new(&mut aligned_uniforms);
    writer.write(noise_uniforms.as_slice()).unwrap();

    aligned_uniforms
}
