use crate::{data, drawer, render, settings};
use render::{
    Buffer, Context, DoubleFramebuffer, Framebuffer, TextureOptions, Uniform, UniformBlock,
    UniformValue, VertexArrayObject,
};
use settings::Settings;

use crevice::std140::AsStd140;
use glow::HasContext;
use half::f16;
use std::cell::Ref;
use std::rc::Rc;

pub struct Fluid {
    context: Context,
    settings: Rc<Settings>,

    pub width: u32,
    pub height: u32,
    scaling_ratio: drawer::ScalingRatio,
    texel_size: [f32; 2],

    uniforms: UniformBlock<FluidUniforms>,
    vertex_buffer: VertexArrayObject,
    #[allow(unused)]
    plane_vertices: Buffer,

    velocity_textures: DoubleFramebuffer,
    advection_forward_texture: Framebuffer,
    advection_reverse_texture: Framebuffer,
    divergence_texture: Framebuffer,
    pressure_textures: DoubleFramebuffer,

    clear_pressure_to_pass: render::Program,
    advection_pass: render::Program,
    adjust_advection_pass: render::Program,
    diffusion_pass: render::Program,
    divergence_pass: render::Program,
    pressure_pass: render::Program,
    subtract_gradient_pass: render::Program,
}

impl Fluid {
    pub fn new(
        context: &Context,
        scaling_ratio: drawer::ScalingRatio,
        settings: &Rc<Settings>,
    ) -> Result<Self, render::Problem> {
        log::info!("💧 Condensing fluid");

        let (width, height) = (
            scaling_ratio.rounded_x() * settings.fluid_size,
            scaling_ratio.rounded_y() * settings.fluid_size,
        );
        let texel_size = [1.0 / width as f32, 1.0 / height as f32];

        // Framebuffers
        let half_float_zero = f16::from_f32(0.0);
        let zero_array_of_r16 = vec![half_float_zero; (width * height) as usize];
        let zero_array_of_rg16 = vec![half_float_zero; (2 * width * height) as usize];

        let velocity_textures = render::DoubleFramebuffer::new(
            context,
            width,
            height,
            TextureOptions {
                mag_filter: glow::LINEAR,
                min_filter: glow::LINEAR,
                format: glow::RG16F,
                ..Default::default()
            },
        )?;
        velocity_textures.with_data(Some(&zero_array_of_rg16))?;

        let advection_forward_texture = render::Framebuffer::new(
            context,
            width,
            height,
            TextureOptions {
                mag_filter: glow::LINEAR,
                min_filter: glow::LINEAR,
                format: glow::RG16F,
                ..Default::default()
            },
        )?;
        advection_forward_texture.with_data(Some(&zero_array_of_rg16))?;

        let advection_reverse_texture = render::Framebuffer::new(
            context,
            width,
            height,
            TextureOptions {
                mag_filter: glow::LINEAR,
                min_filter: glow::LINEAR,
                format: glow::RG16F,
                ..Default::default()
            },
        )?;
        advection_reverse_texture.with_data(Some(&zero_array_of_rg16))?;

        let divergence_texture = render::Framebuffer::new(
            context,
            width,
            height,
            TextureOptions {
                mag_filter: glow::LINEAR,
                min_filter: glow::LINEAR,
                format: glow::R16F,
                ..Default::default()
            },
        )?;
        divergence_texture.with_data(Some(&zero_array_of_r16))?;

        let pressure_textures = render::DoubleFramebuffer::new(
            context,
            width,
            height,
            TextureOptions {
                mag_filter: glow::LINEAR,
                min_filter: glow::LINEAR,
                format: glow::R16F,
                ..Default::default()
            },
        )?;
        pressure_textures.with_data(Some(&zero_array_of_r16))?;

        // Geometry
        let plane_vertices = Buffer::from_f32(
            context,
            &data::PLANE_VERTICES,
            glow::ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;

        let clear_pressure_to_pass = render::Program::new(
            context,
            (CLEAR_PRESSURE_TO_VERT_SHADER, CLEAR_PRESSURE_TO_FRAG_SHADER),
        )?;
        let advection_pass =
            render::Program::new(context, (FLUID_VERT_SHADER, ADVECTION_FRAG_SHADER))?;
        let adjust_advection_pass =
            render::Program::new(context, (FLUID_VERT_SHADER, ADJUST_ADVECTION_FRAG_SHADER))?;
        let diffusion_pass =
            render::Program::new(context, (FLUID_VERT_SHADER, DIFFUSE_FRAG_SHADER))?;
        let divergence_pass =
            render::Program::new(context, (FLUID_VERT_SHADER, DIVERGENCE_FRAG_SHADER))?;
        let pressure_pass =
            render::Program::new(context, (FLUID_VERT_SHADER, SOLVE_PRESSURE_FRAG_SHADER))?;
        let subtract_gradient_pass =
            render::Program::new(context, (FLUID_VERT_SHADER, SUBTRACT_GRADIENT_FRAG_SHADER))?;

        let uniforms = UniformBlock::new(
            context,
            FluidUniforms {
                timestep: 1.0 / settings.fluid_timestep,
                dissipation: settings.velocity_dissipation,
                texel_size: texel_size.into(),
            },
            0,
            glow::DYNAMIC_DRAW,
        )?;

        advection_pass.set_uniform_block("FluidUniforms", uniforms.index);
        adjust_advection_pass.set_uniform_block("FluidUniforms", uniforms.index);
        diffusion_pass.set_uniform_block("FluidUniforms", uniforms.index);
        divergence_pass.set_uniform_block("FluidUniforms", uniforms.index);
        pressure_pass.set_uniform_block("FluidUniforms", uniforms.index);
        subtract_gradient_pass.set_uniform_block("FluidUniforms", uniforms.index);

        advection_pass.set_uniforms(&[&Uniform {
            name: "velocityTexture",
            value: UniformValue::Texture2D(0),
        }]);
        adjust_advection_pass.set_uniforms(&[
            &Uniform {
                name: "velocityTexture",
                value: UniformValue::Texture2D(0),
            },
            &Uniform {
                name: "forwardAdvectedTexture",
                value: UniformValue::Texture2D(1),
            },
            &Uniform {
                name: "reverseAdvectedTexture",
                value: UniformValue::Texture2D(2),
            },
        ]);
        diffusion_pass.set_uniforms(&[&Uniform {
            name: "velocityTexture",
            value: UniformValue::Texture2D(0),
        }]);
        divergence_pass.set_uniform(&Uniform {
            name: "velocityTexture",
            value: UniformValue::Texture2D(0),
        });

        // a = -dx^2
        let alpha = -1.0;
        let r_beta = 0.25;
        pressure_pass.set_uniforms(&[
            &Uniform {
                name: "divergenceTexture",
                value: UniformValue::Texture2D(0),
            },
            &Uniform {
                name: "pressureTexture",
                value: UniformValue::Texture2D(1),
            },
            &Uniform {
                name: "alpha",
                value: UniformValue::Float(alpha),
            },
            &Uniform {
                name: "rBeta",
                value: UniformValue::Float(r_beta),
            },
        ]);

        subtract_gradient_pass.set_uniforms(&[
            &Uniform {
                name: "velocityTexture",
                value: UniformValue::Texture2D(0),
            },
            &Uniform {
                name: "pressureTexture",
                value: UniformValue::Texture2D(1),
            },
        ]);

        let vertex_buffer = VertexArrayObject::new(
            context,
            &advection_pass,
            &[(
                &plane_vertices,
                render::VertexBufferLayout {
                    name: "position",
                    size: 2,
                    type_: glow::FLOAT,
                    ..Default::default()
                },
            )],
            None,
        )?;

        Ok(Self {
            context: Rc::clone(context),
            settings: Rc::clone(settings),

            width,
            height,
            scaling_ratio,
            texel_size,

            uniforms,
            vertex_buffer,
            plane_vertices,

            velocity_textures,
            advection_forward_texture,
            advection_reverse_texture,
            divergence_texture,
            pressure_textures,

            clear_pressure_to_pass,
            advection_pass,
            adjust_advection_pass,
            diffusion_pass,
            divergence_pass,
            pressure_pass,
            subtract_gradient_pass,
        })
    }

    pub fn resize(&mut self, scaling_ratio: drawer::ScalingRatio) -> Result<(), render::Problem> {
        let (width, height) = (
            scaling_ratio.rounded_x() * self.settings.fluid_size,
            scaling_ratio.rounded_y() * self.settings.fluid_size,
        );

        if (self.width, self.height) != (width, height) {
            self.resize_fluid_texture(width, height)?;
        }

        Ok(())
    }

    pub fn update(&mut self, new_settings: &Rc<Settings>) {
        if self.settings.fluid_size != new_settings.fluid_size {
            let (width, height) = (
                self.scaling_ratio.rounded_x() * self.settings.fluid_size,
                self.scaling_ratio.rounded_y() * self.settings.fluid_size,
            );
            self.resize_fluid_texture(width, height).unwrap();
        }

        self.uniforms
            .update(|data| {
                data.dissipation = new_settings.velocity_dissipation;
            })
            .buffer_data();

        self.settings = Rc::clone(new_settings); // Fix
    }

    pub fn resize_fluid_texture(&mut self, width: u32, height: u32) -> Result<(), render::Problem> {
        self.width = width;
        self.height = height;
        self.texel_size = [1.0 / width as f32, 1.0 / height as f32];
        self.uniforms
            .update(|data| {
                data.texel_size = self.texel_size.into();
            })
            .buffer_data();

        // Create new textures and copy the old contents over
        let velocity_textures = render::DoubleFramebuffer::new(
            &self.context,
            self.width,
            self.height,
            self.velocity_textures.current().options,
        )?;
        velocity_textures.with_data(None::<&[f32]>)?;
        self.velocity_textures
            .blit_to(&self.context, &velocity_textures);
        self.velocity_textures = velocity_textures;

        let divergence_texture = render::Framebuffer::new(
            &self.context,
            self.width,
            self.height,
            self.divergence_texture.options,
        )?;
        divergence_texture.with_data(None::<&[f32]>)?;
        self.divergence_texture
            .blit_to(&self.context, &divergence_texture);
        self.divergence_texture = divergence_texture;

        let pressure_textures = render::DoubleFramebuffer::new(
            &self.context,
            self.width,
            self.height,
            self.pressure_textures.current().options,
        )?;
        pressure_textures.with_data(None::<&[f32]>)?;
        self.pressure_textures
            .blit_to(&self.context, &pressure_textures);
        self.pressure_textures = pressure_textures;

        Ok(())
    }

    pub fn advect_forward(&self, timestep: f32) {
        self.advection_forward_texture
            .draw_to(&self.context, || unsafe {
                self.advection_pass.use_program();
                self.vertex_buffer.bind();
                self.uniforms.bind();

                self.advection_pass.set_uniform(&Uniform {
                    name: "amount",
                    value: UniformValue::Float(timestep),
                });

                self.context.active_texture(glow::TEXTURE0);
                self.context.bind_texture(
                    glow::TEXTURE_2D,
                    Some(self.velocity_textures.current().texture),
                );

                self.context.draw_arrays(glow::TRIANGLES, 0, 6);
            });
    }

    pub fn advect_reverse(&self, timestep: f32) {
        self.advection_reverse_texture
            .draw_to(&self.context, || unsafe {
                self.advection_pass.use_program();
                self.vertex_buffer.bind();
                self.uniforms.bind();

                self.advection_pass.set_uniform(&Uniform {
                    name: "amount",
                    value: UniformValue::Float(-timestep),
                });

                self.context.active_texture(glow::TEXTURE0);
                self.context.bind_texture(
                    glow::TEXTURE_2D,
                    Some(self.advection_forward_texture.texture),
                );

                self.context.draw_arrays(glow::TRIANGLES, 0, 6);
            });
    }

    pub fn adjust_advection(&self, timestep: f32) {
        self.velocity_textures
            .draw_to(&self.context, |velocity_texture| unsafe {
                self.adjust_advection_pass.use_program();
                self.vertex_buffer.bind();

                self.adjust_advection_pass.set_uniform(&Uniform {
                    name: "deltaTime",
                    value: UniformValue::Float(timestep),
                });

                self.context.active_texture(glow::TEXTURE0);
                self.context
                    .bind_texture(glow::TEXTURE_2D, Some(velocity_texture.texture));

                self.context.active_texture(glow::TEXTURE1);
                self.context.bind_texture(
                    glow::TEXTURE_2D,
                    Some(self.advection_forward_texture.texture),
                );

                self.context.active_texture(glow::TEXTURE2);
                self.context.bind_texture(
                    glow::TEXTURE_2D,
                    Some(self.advection_reverse_texture.texture),
                );

                self.context.draw_arrays(glow::TRIANGLES, 0, 6);
            });
    }

    pub fn diffuse(&self, timestep: f32) {
        self.diffusion_pass.use_program();
        self.vertex_buffer.bind();

        // dx^2 / (rho * dt)
        let center_factor = 1.0 / (self.settings.viscosity * timestep);
        let stencil_factor = 1.0 / (4.0 + center_factor);

        self.diffusion_pass.set_uniforms(&[
            &Uniform {
                name: "alpha",
                value: UniformValue::Float(center_factor),
            },
            &Uniform {
                name: "rBeta",
                value: UniformValue::Float(stencil_factor),
            },
        ]);

        for _ in 0..self.settings.diffusion_iterations {
            self.velocity_textures
                .draw_to(&self.context, |velocity_texture| unsafe {
                    self.context.active_texture(glow::TEXTURE0);
                    self.context
                        .bind_texture(glow::TEXTURE_2D, Some(velocity_texture.texture));

                    self.context.draw_arrays(glow::TRIANGLES, 0, 6);
                });
        }
    }

    pub fn calculate_divergence(&self) {
        self.divergence_texture.draw_to(&self.context, || unsafe {
            self.divergence_pass.use_program();
            self.vertex_buffer.bind();
            self.uniforms.bind();

            self.context.active_texture(glow::TEXTURE0);
            self.context.bind_texture(
                glow::TEXTURE_2D,
                Some(self.velocity_textures.current().texture),
            );

            self.context.draw_arrays(glow::TRIANGLES, 0, 6);
        });
    }

    pub fn clear_pressure(&self, pressure: f32) {
        self.clear_pressure_to_pass.use_program();
        self.vertex_buffer.bind();
        self.clear_pressure_to_pass.set_uniform(&Uniform {
            name: "uClearPressure",
            value: UniformValue::Float(pressure),
        });
        let draw_quad = || unsafe {
            self.context.draw_arrays(glow::TRIANGLES, 0, 6);
        };
        self.pressure_textures
            .current()
            .draw_to(&self.context, draw_quad);
        self.pressure_textures
            .next()
            .draw_to(&self.context, draw_quad);
    }

    pub fn solve_pressure(&self) {
        use settings::PressureMode::*;
        match self.settings.pressure_mode {
            ClearWith(pressure) => {
                self.clear_pressure(pressure);
            }
            Retain => (),
        }

        self.pressure_pass.use_program();
        unsafe {
            self.vertex_buffer.bind();
            self.context.active_texture(glow::TEXTURE0);
            self.context
                .bind_texture(glow::TEXTURE_2D, Some(self.divergence_texture.texture));
        }

        for _ in 0..self.settings.pressure_iterations {
            self.pressure_textures
                .draw_to(&self.context, |pressure_texture| unsafe {
                    self.context.active_texture(glow::TEXTURE1);
                    self.context
                        .bind_texture(glow::TEXTURE_2D, Some(pressure_texture.texture));

                    self.context.draw_arrays(glow::TRIANGLES, 0, 6);
                });
        }
    }

    pub fn subtract_gradient(&self) {
        self.velocity_textures
            .draw_to(&self.context, |velocity_texture| unsafe {
                self.subtract_gradient_pass.use_program();
                self.vertex_buffer.bind();
                self.uniforms.bind();

                self.context.active_texture(glow::TEXTURE0);
                self.context
                    .bind_texture(glow::TEXTURE_2D, Some(velocity_texture.texture));
                self.context.active_texture(glow::TEXTURE1);
                self.context.bind_texture(
                    glow::TEXTURE_2D,
                    Some(self.pressure_textures.current().texture),
                );

                self.context.draw_arrays(glow::TRIANGLES, 0, 6);
            });
    }

    pub fn get_velocity(&self) -> Ref<Framebuffer> {
        self.velocity_textures.current()
    }

    pub fn get_divergence(&self) -> &Framebuffer {
        &self.divergence_texture
    }

    pub fn get_pressure(&self) -> Ref<Framebuffer> {
        self.pressure_textures.current()
    }

    pub fn get_velocity_textures(&self) -> &DoubleFramebuffer {
        &self.velocity_textures
    }
}

#[derive(Copy, Clone, Debug, AsStd140)]
struct FluidUniforms {
    timestep: f32,
    dissipation: f32,
    texel_size: mint::Vector2<f32>,
}

static CLEAR_PRESSURE_TO_VERT_SHADER: &str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/clear_pressure_to.vert"));
static CLEAR_PRESSURE_TO_FRAG_SHADER: &str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/clear_pressure_to.frag"));
static FLUID_VERT_SHADER: &str = include_str!(concat!(env!("OUT_DIR"), "/shaders/fluid.vert"));
static ADVECTION_FRAG_SHADER: &str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/advection.frag"));
static ADJUST_ADVECTION_FRAG_SHADER: &str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/adjust_advection.frag"));
static DIFFUSE_FRAG_SHADER: &str = include_str!(concat!(env!("OUT_DIR"), "/shaders/diffuse.frag"));
static DIVERGENCE_FRAG_SHADER: &str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/divergence.frag"));
static SOLVE_PRESSURE_FRAG_SHADER: &str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/solve_pressure.frag"));
static SUBTRACT_GRADIENT_FRAG_SHADER: &str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/subtract_gradient.frag"));
