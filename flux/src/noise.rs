use crate::{data, render, settings};
use render::{
    Buffer, Context, DoubleFramebuffer, Framebuffer, Program, TextureOptions, Uniform,
    UniformValue, VertexBufferLayout,
};
use settings::Noise;

use bytemuck::{Pod, Zeroable};
use std::rc::Rc;
use web_sys::WebGl2RenderingContext as GL;
use web_sys::WebGlVertexArrayObject;

static NOISE_VERT_SHADER: &'static str = include_str!("./shaders/noise.vert");
static SIMPLEX_NOISE_FRAG_SHADER: &'static str = include_str!("./shaders/simplex_noise.frag");
static BLEND_WITH_CURL: &'static str = include_str!("./shaders/blend_with_curl.frag");
static BLEND_WITH_WIGGLE: &'static str = include_str!("./shaders/blend_with_wiggle.frag");

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct NoiseUniforms {
    frequency: f32,
    offset_1: f32,
    offset_2: f32,
    multiplier: f32,

    texel_size: [f32; 2],
    pad1: f32,
    pad2: f32,
}

pub struct NoiseChannel {
    noise: Noise,
    texture: Framebuffer,
    blend_begin_time: f32,
    last_blend_progress: f32,
    offset1: f32,
    offset2: f32,
    uniforms: Buffer,
}

impl NoiseChannel {
    pub fn tick(&mut self, context: &Context, elapsed_time: f32) -> () {
        self.blend_begin_time = elapsed_time;
        self.last_blend_progress = 0.0;
        self.offset1 += self.noise.offset_increment;
        self.offset2 += self.noise.offset_increment;

        context.bind_buffer(GL::UNIFORM_BUFFER, Some(&self.uniforms.id));
        context.buffer_sub_data_with_i32_and_u8_array(
            GL::UNIFORM_BUFFER,
            1 * 4,
            &bytemuck::bytes_of(&[self.offset1, self.offset2]),
        );
        context.bind_buffer(GL::UNIFORM_BUFFER, None);
    }
}

pub struct NoiseInjector {
    context: Context,
    pub channels: Vec<NoiseChannel>,
    width: u32,
    height: u32,
    generate_noise_pass: Program,
    blend_with_curl_pass: Program,
    blend_with_wiggle_pass: Program,

    noise_buffer: WebGlVertexArrayObject,
}

impl NoiseInjector {
    pub fn update_channel(&mut self, channel_number: usize, noise: &Noise) -> () {
        if let Some(channel) = self.channels.get_mut(channel_number) {
            channel.noise = noise.clone();

            let uniforms = NoiseUniforms {
                frequency: noise.scale,
                offset_1: noise.offset_1,
                offset_2: noise.offset_2,
                multiplier: noise.multiplier,
                texel_size: [1.0 / self.width as f32, 1.0 / self.height as f32],
                pad1: 0.0,
                pad2: 0.0,
            };

            self.context
                .bind_buffer(GL::UNIFORM_BUFFER, Some(&channel.uniforms.id));
            self.context.buffer_sub_data_with_i32_and_u8_array(
                GL::UNIFORM_BUFFER,
                0,
                &bytemuck::bytes_of(&uniforms),
            );
            self.context.bind_buffer(GL::UNIFORM_BUFFER, None);
        }
    }

    pub fn new(context: &Context, width: u32, height: u32) -> Result<Self, render::Problem> {
        // Geometry
        let plane_vertices = Buffer::from_f32(
            &context,
            &data::PLANE_VERTICES.to_vec(), // fix
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;
        let plane_indices = Buffer::from_u16(
            &context,
            &data::PLANE_INDICES.to_vec(),
            GL::ELEMENT_ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;

        let simplex_noise_program =
            Program::new(&context, (NOISE_VERT_SHADER, SIMPLEX_NOISE_FRAG_SHADER))?;
        let blend_with_curl_program = Program::new(&context, (NOISE_VERT_SHADER, BLEND_WITH_CURL))?;
        let blend_with_wiggle_program =
            Program::new(&context, (NOISE_VERT_SHADER, BLEND_WITH_WIGGLE))?;

        let noise_buffer = render::create_vertex_array(
            &context,
            &simplex_noise_program,
            &[(
                &plane_vertices,
                VertexBufferLayout {
                    name: "position",
                    size: 3,
                    type_: GL::FLOAT,
                    ..Default::default()
                },
            )],
            Some(&plane_indices),
        )?;

        simplex_noise_program.set_uniform_block("NoiseUniforms", 3);
        blend_with_curl_program.set_uniform_block("NoiseUniforms", 3);
        blend_with_wiggle_program.set_uniform_block("NoiseUniforms", 3);

        simplex_noise_program.set_uniform(&Uniform {
            name: "uResolution",
            value: UniformValue::Vec2(&[width as f32, height as f32]),
        });

        blend_with_curl_program.set_uniforms(&[
            &Uniform {
                name: "inputTexture",
                value: UniformValue::Texture2D(0),
            },
            &Uniform {
                name: "noiseTexture",
                value: UniformValue::Texture2D(1),
            },
        ]);
        blend_with_wiggle_program.set_uniforms(&[
            &Uniform {
                name: "inputTexture",
                value: UniformValue::Texture2D(0),
            },
            &Uniform {
                name: "noiseTexture",
                value: UniformValue::Texture2D(1),
            },
        ]);

        Ok(Self {
            context: Rc::clone(context),
            channels: Vec::new(),
            width,
            height,
            generate_noise_pass: simplex_noise_program,
            blend_with_curl_pass: blend_with_curl_program,
            blend_with_wiggle_pass: blend_with_wiggle_program,

            noise_buffer,
        })
    }

    pub fn add_noise(&mut self, noise: Noise) -> Result<(), render::Problem> {
        let texture = Framebuffer::new(
            &self.context,
            self.width,
            self.height,
            TextureOptions {
                mag_filter: GL::LINEAR,
                min_filter: GL::LINEAR,
                format: GL::RG32F,
                ..Default::default()
            },
        )?
        .with_f32_data(&vec![0.0; (self.width * self.height * 2) as usize])?;

        let uniforms = NoiseUniforms {
            frequency: noise.scale,
            offset_1: noise.offset_1,
            offset_2: noise.offset_2,
            multiplier: noise.multiplier,
            texel_size: [1.0 / self.width as f32, 1.0 / self.height as f32],
            pad1: 0.0,
            pad2: 0.0,
        };

        let uniforms = Buffer::from_f32_array(
            &self.context,
            &bytemuck::cast_slice(&[uniforms]),
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;

        self.channels.push(NoiseChannel {
            noise: noise.clone(),
            texture,
            blend_begin_time: 0.0,
            last_blend_progress: 0.0,
            offset1: noise.offset_1,
            offset2: noise.offset_2,
            uniforms,
        });

        Ok(())
    }

    pub fn generate_all(&mut self, elapsed_time: f32) -> () {
        for channel in self.channels.iter_mut() {
            let time_since_last_update = elapsed_time - channel.blend_begin_time;

            if time_since_last_update >= channel.noise.delay {
                self.generate_noise_pass.use_program();
                self.context.bind_vertex_array(Some(&self.noise_buffer));

                self.context
                    .bind_buffer_base(GL::UNIFORM_BUFFER, 3, Some(&channel.uniforms.id));

                channel.texture.draw_to(&self.context, || {
                    self.context
                        .draw_elements_with_i32(GL::TRIANGLES, 6, GL::UNSIGNED_SHORT, 0);
                });

                channel.tick(&self.context, elapsed_time);
            }
        }
    }
    pub fn generate_by_channel_number(&mut self, channel_number: usize, elapsed_time: f32) {
        if let Some(channel) = self.channels.get_mut(channel_number) {
            self.generate_noise_pass.use_program();
            self.context.bind_vertex_array(Some(&self.noise_buffer));

            self.generate_noise_pass.set_uniform(&Uniform {
                name: "uResolution",
                value: UniformValue::Vec2(&[self.width as f32, self.height as f32]),
            });

            self.context
                .bind_buffer_base(GL::UNIFORM_BUFFER, 3, Some(&channel.uniforms.id));

            channel.texture.draw_to(&self.context, || {
                self.context
                    .draw_elements_with_i32(GL::TRIANGLES, 6, GL::UNSIGNED_SHORT, 0);
            });

            channel.tick(&self.context, elapsed_time);
        }
    }

    pub fn blend_noise_into(
        &mut self,
        target_textures: &DoubleFramebuffer,
        elapsed_time: f32,
    ) -> () {
        for channel in self.channels.iter_mut() {
            let blend_progress: f32 = ((elapsed_time - channel.blend_begin_time)
                / channel.noise.blend_duration)
                .clamp(0.0, 1.0);

            if blend_progress >= 1.0 - 0.0001 {
                continue;
            }

            let delta_blend_progress = blend_progress - channel.last_blend_progress;
            let blend_pass: &Program = match channel.noise.blend_method {
                settings::BlendMethod::Curl => &self.blend_with_curl_pass,
                settings::BlendMethod::Wiggle => &self.blend_with_wiggle_pass,
            };

            target_textures.draw_to(&self.context, |target_texture| {
                blend_pass.use_program();
                self.context.bind_vertex_array(Some(&self.noise_buffer));

                self.context
                    .bind_buffer_base(GL::UNIFORM_BUFFER, 3, Some(&channel.uniforms.id));

                blend_pass.set_uniform(&Uniform {
                    name: "uBlendProgress",
                    value: UniformValue::Float(delta_blend_progress),
                });

                self.context.active_texture(GL::TEXTURE0);
                self.context
                    .bind_texture(GL::TEXTURE_2D, Some(&target_texture.texture));

                self.context.active_texture(GL::TEXTURE1);
                self.context
                    .bind_texture(GL::TEXTURE_2D, Some(&channel.texture.texture));

                self.context
                    .draw_elements_with_i32(GL::TRIANGLES, 6, GL::UNSIGNED_SHORT, 0);
            });

            channel.last_blend_progress = blend_progress;
        }
    }

    #[allow(dead_code)]
    pub fn get_noise_channel(&self, channel_number: usize) -> Option<&Framebuffer> {
        self.channels
            .get(channel_number)
            .map(|channel| &channel.texture)
    }
}
