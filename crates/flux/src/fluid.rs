use crate::{data, render, settings};
use render::{
    Buffer, Context, DoubleFramebuffer, Framebuffer, TextureOptions, Uniform, UniformValue,
    VertexArrayObject,
};
use settings::Settings;

use crevice::std140::{AsStd140, Std140};
use glow::HasContext;
use half::f16;
use std::cell::Ref;
use std::rc::Rc;

static FLUID_VERT_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/fluid.vert"));
static ADVECTION_FRAG_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/advection.frag"));
static ADJUST_ADVECTION_FRAG_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/adjust_advection.frag"));
static DIFFUSE_FRAG_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/diffuse.frag"));
static DIVERGENCE_FRAG_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/divergence.frag"));
static SOLVE_PRESSURE_FRAG_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/solve_pressure.frag"));
static SUBTRACT_GRADIENT_FRAG_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/subtract_gradient.frag"));

#[derive(Copy, Clone, Debug, AsStd140)]
struct Uniforms {
    timestep: f32,
    dissipation: f32,
    texel_size: mint::Vector2<f32>,
}

pub struct Fluid {
    context: Context,
    settings: Rc<Settings>,

    pub width: u32,
    pub height: u32,
    texel_size: [f32; 2],
    grid_size: f32,

    uniform_buffer: Buffer,
    vertex_buffer: VertexArrayObject,

    velocity_textures: DoubleFramebuffer,
    advection_forward_texture: Framebuffer,
    advection_reverse_texture: Framebuffer,
    divergence_texture: Framebuffer,
    pressure_textures: DoubleFramebuffer,

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
        ratio: f32,
        settings: &Rc<Settings>,
    ) -> Result<Self, render::Problem> {
        // let (width, height, texel_size) = compute_fluid_size(settings.fluid_size as f32, ratio);
        let width = 128;
        let height = 128;
        let texel_size = [1.0 / 128.0, 1.0 / 128.0];
        let grid_size: f32 = 1.0;

        // Framebuffers
        let half_float_zero = f16::from_f32(0.0);
        let zero_array_of_r16 = vec![half_float_zero; (width * height) as usize];
        let zero_array_of_rg16 = vec![half_float_zero; (2 * width * height) as usize];

        let velocity_textures = render::DoubleFramebuffer::new(
            &context,
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
            &context,
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
            &context,
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
            &context,
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
            &context,
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
            &context,
            &data::PLANE_VERTICES,
            glow::ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;
        let plane_indices = Buffer::from_u16(
            &context,
            &data::PLANE_INDICES,
            glow::ELEMENT_ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;

        let advection_program =
            render::Program::new(&context, (FLUID_VERT_SHADER, ADVECTION_FRAG_SHADER))?;
        let adjust_advection_pass =
            render::Program::new(&context, (FLUID_VERT_SHADER, ADJUST_ADVECTION_FRAG_SHADER))?;
        let diffusion_program =
            render::Program::new(&context, (FLUID_VERT_SHADER, DIFFUSE_FRAG_SHADER))?;
        let divergence_program =
            render::Program::new(&context, (FLUID_VERT_SHADER, DIVERGENCE_FRAG_SHADER))?;
        let pressure_program =
            render::Program::new(&context, (FLUID_VERT_SHADER, SOLVE_PRESSURE_FRAG_SHADER))?;
        let subtract_gradient_program =
            render::Program::new(&context, (FLUID_VERT_SHADER, SUBTRACT_GRADIENT_FRAG_SHADER))?;

        let uniforms = Uniforms {
            timestep: 0.0,
            dissipation: settings.velocity_dissipation,
            texel_size: texel_size.into(),
        };

        let uniform_buffer = Buffer::from_bytes(
            &context,
            uniforms.as_std140().as_bytes(),
            glow::ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;

        advection_program.set_uniform_block("FluidUniforms", 0);
        adjust_advection_pass.set_uniform_block("FluidUniforms", 0);
        diffusion_program.set_uniform_block("FluidUniforms", 0);
        divergence_program.set_uniform_block("FluidUniforms", 0);
        pressure_program.set_uniform_block("FluidUniforms", 0);
        subtract_gradient_program.set_uniform_block("FluidUniforms", 0);

        advection_program.set_uniforms(&[&Uniform {
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
        diffusion_program.set_uniforms(&[&Uniform {
            name: "velocityTexture",
            value: UniformValue::Texture2D(0),
        }]);
        divergence_program.set_uniform(&Uniform {
            name: "velocityTexture",
            value: UniformValue::Texture2D(0),
        });

        let alpha = -grid_size * grid_size;
        let r_beta = 0.25;
        pressure_program.set_uniforms(&[
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

        subtract_gradient_program.set_uniforms(&[
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
            &context,
            &advection_program,
            &[(
                &plane_vertices,
                render::VertexBufferLayout {
                    name: "position",
                    size: 3,
                    type_: glow::FLOAT,
                    ..Default::default()
                },
            )],
            Some(&plane_indices),
        )?;

        Ok(Self {
            context: Rc::clone(context),
            settings: Rc::clone(settings),

            width,
            height,
            texel_size,
            grid_size,

            uniform_buffer,
            vertex_buffer,

            velocity_textures,
            advection_forward_texture,
            advection_reverse_texture,
            divergence_texture,
            pressure_textures,

            advection_pass: advection_program,
            adjust_advection_pass,
            diffusion_pass: diffusion_program,
            divergence_pass: divergence_program,
            pressure_pass: pressure_program,
            subtract_gradient_pass: subtract_gradient_program,
        })
    }

    pub fn update(&mut self, settings: &Rc<Settings>) -> () {
        self.settings = Rc::clone(settings); // Fix

        let uniforms = Uniforms {
            timestep: 0.0,
            dissipation: settings.velocity_dissipation,
            texel_size: self.texel_size.into(),
        };

        unsafe {
            self.context
                .bind_buffer(glow::UNIFORM_BUFFER, Some(self.uniform_buffer.id));
            self.context.buffer_sub_data_u8_slice(
                glow::UNIFORM_BUFFER,
                0,
                uniforms.as_std140().as_bytes(),
            );
            self.context.bind_buffer(glow::UNIFORM_BUFFER, None);
        }
    }

    pub fn resize(&mut self, ratio: f32) -> Result<(), render::Problem> {
        let (width, height, texel_size) =
            compute_fluid_size(self.settings.fluid_size as f32, ratio);
        self.width = width;
        self.height = height;
        self.texel_size = texel_size;

        // Update texel size
        unsafe {
            self.context
                .bind_buffer(glow::UNIFORM_BUFFER, Some(self.uniform_buffer.id));
            self.context.buffer_sub_data_u8_slice(
                glow::UNIFORM_BUFFER,
                4 * 4,
                &bytemuck::bytes_of(&texel_size),
            );
            self.context.bind_buffer(glow::UNIFORM_BUFFER, None);
        }

        // Create new textures and copy the old contents over
        let velocity_textures = render::DoubleFramebuffer::new(
            &self.context,
            width,
            height,
            self.velocity_textures.current().options,
        )?;
        velocity_textures.with_data(None::<&[f32]>)?;
        self.velocity_textures
            .blit_to(&self.context, &velocity_textures);
        self.velocity_textures = velocity_textures;

        let divergence_texture = render::Framebuffer::new(
            &self.context,
            width,
            height,
            self.divergence_texture.options,
        )?;
        divergence_texture.with_data(None::<&[f32]>)?;
        self.divergence_texture
            .blit_to(&self.context, &divergence_texture);
        self.divergence_texture = divergence_texture;

        let pressure_textures = render::DoubleFramebuffer::new(
            &self.context,
            width,
            height,
            self.pressure_textures.current().options,
        )?;
        pressure_textures.with_data(None::<&[f32]>)?;
        self.pressure_textures
            .blit_to(&self.context, &pressure_textures);
        self.pressure_textures = pressure_textures;

        Ok(())
    }

    // Setup vertex and uniform buffers.
    pub fn prepare_pass(&self, timestep: f32) {
        unsafe {
            // Update the timestep
            self.context
                .bind_buffer(glow::UNIFORM_BUFFER, Some(self.uniform_buffer.id));
            self.context.buffer_sub_data_u8_slice(
                glow::UNIFORM_BUFFER,
                0,
                &bytemuck::bytes_of(&timestep),
            );
            self.context.bind_buffer(glow::UNIFORM_BUFFER, None);

            self.context.bind_vertex_array(Some(self.vertex_buffer.id));

            self.context
                .bind_buffer_base(glow::UNIFORM_BUFFER, 0, Some(self.uniform_buffer.id));
        }
    }

    pub fn advect_forward(&self) -> () {
        self.advection_forward_texture
            .draw_to(&self.context, || unsafe {
                self.advection_pass.use_program();

                self.advection_pass.set_uniform(&Uniform {
                    name: "amount",
                    value: UniformValue::Float(0.017),
                });

                self.context.active_texture(glow::TEXTURE0);
                self.context.bind_texture(
                    glow::TEXTURE_2D,
                    Some(self.velocity_textures.current().texture),
                );

                self.context
                    .draw_elements(glow::TRIANGLES, 6, glow::UNSIGNED_SHORT, 0);
            });
    }

    pub fn advect_reverse(&self) -> () {
        self.advection_reverse_texture
            .draw_to(&self.context, || unsafe {
                self.advection_pass.use_program();

                self.advection_pass.set_uniform(&Uniform {
                    name: "amount",
                    value: UniformValue::Float(-0.017),
                });

                self.context.active_texture(glow::TEXTURE0);
                self.context.bind_texture(
                    glow::TEXTURE_2D,
                    Some(self.velocity_textures.current().texture),
                );

                self.context
                    .draw_elements(glow::TRIANGLES, 6, glow::UNSIGNED_SHORT, 0);
            });
    }

    pub fn adjust_advection(&self) -> () {
        self.velocity_textures
            .draw_to(&self.context, |velocity_texture| unsafe {
                self.adjust_advection_pass.use_program();

                self.adjust_advection_pass.set_uniform(&&Uniform {
                    name: "deltaTime",
                    value: UniformValue::Float(0.017),
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

                self.context
                    .draw_elements(glow::TRIANGLES, 6, glow::UNSIGNED_SHORT, 0);
            });
    }

    pub fn diffuse(&self, timestep: f32) -> () {
        self.diffusion_pass.use_program();

        let center_factor = self.grid_size * self.grid_size / (self.settings.viscosity * timestep);
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

                    self.context
                        .draw_elements(glow::TRIANGLES, 6, glow::UNSIGNED_SHORT, 0);
                });
        }
    }

    pub fn calculate_divergence(&self) -> () {
        self.divergence_texture.draw_to(&self.context, || unsafe {
            self.divergence_pass.use_program();

            self.context.active_texture(glow::TEXTURE0);
            self.context.bind_texture(
                glow::TEXTURE_2D,
                Some(self.velocity_textures.current().texture),
            );

            self.context
                .draw_elements(glow::TRIANGLES, 6, glow::UNSIGNED_SHORT, 0);
        });
    }

    pub fn solve_pressure(&self) -> () {
        self.pressure_textures
            .clear_color_with(&[self.settings.starting_pressure, 0.0, 0.0, 1.0])
            .unwrap();

        self.pressure_pass.use_program();
        unsafe {
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

                    self.context
                        .draw_elements(glow::TRIANGLES, 6, glow::UNSIGNED_SHORT, 0);
                });
        }
    }

    pub fn subtract_gradient(&self) -> () {
        self.subtract_gradient_pass.use_program();

        self.velocity_textures
            .draw_to(&self.context, |velocity_texture| unsafe {
                self.context.active_texture(glow::TEXTURE0);
                self.context
                    .bind_texture(glow::TEXTURE_2D, Some(velocity_texture.texture));
                self.context.active_texture(glow::TEXTURE1);
                self.context.bind_texture(
                    glow::TEXTURE_2D,
                    Some(self.pressure_textures.current().texture),
                );

                self.context
                    .draw_elements(glow::TRIANGLES, 6, glow::UNSIGNED_SHORT, 0);
            });
    }

    #[allow(dead_code)]
    pub fn get_velocity(&self) -> Ref<Framebuffer> {
        self.velocity_textures.current()
    }

    #[allow(dead_code)]
    pub fn get_divergence(&self) -> &Framebuffer {
        &self.divergence_texture
    }

    #[allow(dead_code)]
    pub fn get_pressure(&self) -> Ref<Framebuffer> {
        self.pressure_textures.current()
    }

    #[allow(dead_code)]
    pub fn get_velocity_textures(&self) -> &DoubleFramebuffer {
        &self.velocity_textures
    }
}

fn compute_fluid_size(fluid_size: f32, ratio: f32) -> (u32, u32, [f32; 2]) {
    let width = (fluid_size * ratio).round();
    let height = fluid_size;
    let texel_size = [1.0 / width, 1.0 / height];

    (width as u32, height as u32, texel_size)
}
