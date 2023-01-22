use crate::{data, drawer, render, rng, settings};
use render::{
    Buffer, Context, DoubleFramebuffer, Framebuffer, Program, TextureOptions, Uniform,
    UniformArray, UniformBlock, UniformValue, VertexArrayObject, VertexBufferLayout,
};

use crevice::std140::AsStd140;
use glow::HasContext;
use half::f16;
use std::rc::Rc;

pub struct NoiseGenerator {
    context: Context,
    channels: Vec<NoiseChannel>,
    texture: Framebuffer,
    scaling_ratio: drawer::ScalingRatio,

    generate_noise_pass: Program,
    inject_noise_pass: Program,

    noise_buffer: VertexArrayObject,
    uniforms: UniformBlock<UniformArray<NoiseUniforms>>,
    #[allow(unused)]
    plane_vertices: Buffer,
}

impl NoiseGenerator {
    pub fn resize(
        &mut self,
        size: u32,
        scaling_ratio: drawer::ScalingRatio,
    ) -> Result<(), render::Problem> {
        if scaling_ratio == self.scaling_ratio {
            return Ok(());
        }

        self.scaling_ratio = scaling_ratio;
        let (width, height) = (
            size * self.scaling_ratio.rounded_x(),
            size * self.scaling_ratio.rounded_y(),
        );
        self.texture = Framebuffer::new(
            &self.context,
            width,
            height,
            TextureOptions {
                mag_filter: glow::LINEAR,
                min_filter: glow::LINEAR,
                format: glow::RG16F,
                ..Default::default()
            },
        )?;
        self.texture.with_data(None::<&[f16]>)
    }

    pub fn update(&mut self, new_settings: &[settings::Noise]) {
        for (channel, new_setting) in self.channels.iter_mut().zip(new_settings.iter()) {
            channel.settings = new_setting.clone();
        }
    }

    pub fn generate(&mut self, elapsed_time: f32) {
        self.uniforms
            .update(|noise_uniforms| {
                *noise_uniforms = UniformArray(
                    self.channels
                        .iter()
                        .map(|channel| NoiseUniforms::new(self.scaling_ratio, channel))
                        .collect(),
                )
            })
            .buffer_data();

        self.generate_noise_pass.use_program();

        unsafe {
            self.noise_buffer.bind();
            self.uniforms.bind();

            self.texture.draw_to(&self.context, || {
                self.context.draw_arrays(glow::TRIANGLES, 0, 6);
            });
        }

        for channel in self.channels.iter_mut() {
            channel.tick(elapsed_time);
        }
    }

    pub fn blend_noise_into(&mut self, target_textures: &DoubleFramebuffer, timestep: f32) {
        target_textures.draw_to(&self.context, |target_texture| {
            self.inject_noise_pass.use_program();

            unsafe {
                self.context.disable(glow::BLEND);
                self.noise_buffer.bind();

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

                self.context.draw_arrays(glow::TRIANGLES, 0, 6);
            }
        });
    }

    pub fn get_noise(&self) -> &Framebuffer {
        &self.texture
    }
}

pub struct NoiseGeneratorBuilder {
    context: Context,
    size: u32,
    scaling_ratio: drawer::ScalingRatio,
    channels: Vec<NoiseChannel>,
}

impl NoiseGeneratorBuilder {
    pub fn new(context: &Context, size: u32, scaling_ratio: drawer::ScalingRatio) -> Self {
        NoiseGeneratorBuilder {
            context: Rc::clone(context),
            size,
            scaling_ratio,
            channels: Vec::new(),
        }
    }

    pub fn add_channel(&mut self, channel: &settings::Noise) -> &Self {
        self.channels.push(NoiseChannel {
            settings: channel.clone(),
            scale: channel.scale,
            offset_1: 4.0 * rng::gen::<f32>(),
            offset_2: 0.0,
            blend_factor: 0.0,
        });

        self
    }

    pub fn build(self) -> Result<NoiseGenerator, render::Problem> {
        log::info!("🎛 Generating noise");

        let (width, height) = (
            self.size * self.scaling_ratio.rounded_x(),
            self.size * self.scaling_ratio.rounded_y(),
        );

        // Geometry
        let plane_vertices = Buffer::from_f32(
            &self.context,
            &data::PLANE_VERTICES,
            glow::ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;
        let texture = Framebuffer::new(
            &self.context,
            width,
            height,
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
                    size: 2,
                    type_: glow::FLOAT,
                    ..Default::default()
                },
            )],
            None,
        )?;

        let uniforms = UniformBlock::new(
            &self.context,
            UniformArray(
                self.channels
                    .iter()
                    .map(|channel| NoiseUniforms::new(self.scaling_ratio, channel))
                    .collect(),
            ),
            0,
            glow::DYNAMIC_DRAW,
        )?;

        generate_noise_pass.set_uniform_block("Channels", uniforms.index);
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
            scaling_ratio: self.scaling_ratio,

            generate_noise_pass,
            inject_noise_pass,

            noise_buffer,
            uniforms,
            plane_vertices,
        })
    }
}

pub struct NoiseChannel {
    settings: settings::Noise,
    scale: f32,
    offset_1: f32,
    offset_2: f32,
    blend_factor: f32,
}

impl NoiseChannel {
    pub fn tick(&mut self, elapsed_time: f32) {
        const BLEND_THRESHOLD: f32 = 20.0;

        self.scale = self.settings.scale
            * (1.0 + 0.15 * (0.01 * elapsed_time * std::f32::consts::TAU).sin());
        self.offset_1 += self.settings.offset_increment;

        if self.offset_1 > BLEND_THRESHOLD {
            self.blend_factor += self.settings.offset_increment;
            self.offset_2 += self.settings.offset_increment;
        }

        // Reset blending
        if self.blend_factor > 1.0 {
            self.offset_1 = self.offset_2;
            self.offset_2 = 0.0;
            self.blend_factor = 0.0;
        }
    }
}

#[derive(AsStd140)]
pub struct NoiseUniforms {
    scale: mint::Vector2<f32>,
    offset_1: f32,
    offset_2: f32,
    blend_factor: f32,
    multiplier: f32,
}

impl NoiseUniforms {
    fn new(scaling_ratio: drawer::ScalingRatio, channel: &NoiseChannel) -> Self {
        Self {
            scale: [
                channel.scale * scaling_ratio.x(),
                channel.scale * scaling_ratio.y(),
            ]
            .into(),
            offset_1: channel.offset_1,
            offset_2: channel.offset_2,
            blend_factor: channel.blend_factor,
            multiplier: channel.settings.multiplier,
        }
    }
}

static NOISE_VERT_SHADER: &str = include_str!(concat!(env!("OUT_DIR"), "/shaders/noise.vert"));
static GENERATE_NOISE_FRAG_SHADER: &str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/generate_noise.frag"));
static INJECT_NOISE_FRAG_SHADER: &str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/inject_noise.frag"));
