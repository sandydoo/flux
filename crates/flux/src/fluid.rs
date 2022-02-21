use crate::{data, render, settings};
use render::{
    Buffer, Context, DoubleFramebuffer, Framebuffer, TextureOptions, Uniform, UniformValue,
    VertexArrayObject,
};
use settings::Settings;

use bytemuck::{Pod, Zeroable};
use glow::HasContext;
use half::f16;
use std::cell::Ref;
use std::rc::Rc;

static FLUID_VERT_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/fluid.vert"));
static ADVECTION_FRAG_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/advection.frag"));
static DIVERGENCE_FRAG_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/divergence.frag"));
static SOLVE_PRESSURE_FRAG_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/solve_pressure.frag"));
static SUBTRACT_GRADIENT_FRAG_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/subtract_gradient.frag"));

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    timestep: f32,
    epsilon: f32,
    half_epsilon: f32,
    dissipation: f32,
    texel_size: [f32; 2],
    pad1: f32,
    pad2: f32,
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
    divergence_texture: Framebuffer,
    pressure_textures: DoubleFramebuffer,

    advection_pass: render::Program,
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
        let grid_size: f32 = 1.0;
        let (width, height, texel_size) = compute_fluid_size(settings.fluid_size as f32, ratio);

        // Framebuffers
        let zero_in_f16 = f16::from_f32(0.0);
        let mut initial_data: Vec<f16> = Vec::with_capacity((2 * width * height) as usize);
        for _ in 0..initial_data.capacity() {
            initial_data.push(zero_in_f16);
        }

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
        )?
        .with_data(Some(&initial_data))?;

        let divergence_texture = render::Framebuffer::new(
            &context,
            width,
            height,
            TextureOptions {
                mag_filter: glow::LINEAR,
                min_filter: glow::LINEAR,
                format: glow::RG16F,
                ..Default::default()
            },
        )?
        .with_data(Some(&initial_data))?;

        let pressure_textures = render::DoubleFramebuffer::new(
            &context,
            width,
            height,
            TextureOptions {
                mag_filter: glow::LINEAR,
                min_filter: glow::LINEAR,
                format: glow::RG16F,
                ..Default::default()
            },
        )?
        .with_data(Some(&initial_data))?;

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
        let divergence_program =
            render::Program::new(&context, (FLUID_VERT_SHADER, DIVERGENCE_FRAG_SHADER))?;
        let pressure_program =
            render::Program::new(&context, (FLUID_VERT_SHADER, SOLVE_PRESSURE_FRAG_SHADER))?;
        let diffusion_program = pressure_program.clone();
        let subtract_gradient_program =
            render::Program::new(&context, (FLUID_VERT_SHADER, SUBTRACT_GRADIENT_FRAG_SHADER))?;

        let uniforms = Uniforms {
            timestep: 0.0,
            epsilon: grid_size,
            half_epsilon: 0.5 * grid_size,
            dissipation: settings.velocity_dissipation,
            texel_size,
            pad1: 0.0,
            pad2: 0.0,
        };

        let uniform_buffer = Buffer::from_f32(
            &context,
            &bytemuck::cast_slice(&[uniforms]),
            glow::ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;

        advection_program.set_uniform_block("FluidUniforms", 0);
        diffusion_program.set_uniform_block("FluidUniforms", 0);
        divergence_program.set_uniform_block("FluidUniforms", 0);
        pressure_program.set_uniform_block("FluidUniforms", 0);
        subtract_gradient_program.set_uniform_block("FluidUniforms", 0);

        // TODO can I add this to the uniform buffer? Is that even worth it?
        advection_program.set_uniforms(&[
            &Uniform {
                name: "inputTexture",
                value: UniformValue::Texture2D(0),
            },
            &Uniform {
                name: "velocityTexture",
                value: UniformValue::Texture2D(1),
            },
        ]);
        diffusion_program.set_uniforms(&[
            &Uniform {
                name: "divergenceTexture",
                value: UniformValue::Texture2D(0),
            },
            &Uniform {
                name: "pressureTexture",
                value: UniformValue::Texture2D(1),
            },
        ]);
        divergence_program.set_uniform(&Uniform {
            name: "velocityTexture",
            value: UniformValue::Texture2D(0),
        });
        pressure_program.set_uniforms(&[
            &Uniform {
                name: "divergenceTexture",
                value: UniformValue::Texture2D(0),
            },
            &Uniform {
                name: "pressureTexture",
                value: UniformValue::Texture2D(1),
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
            divergence_texture,
            pressure_textures,

            advection_pass: advection_program,
            diffusion_pass: pressure_program.clone(),
            divergence_pass: divergence_program,
            pressure_pass: pressure_program,
            subtract_gradient_pass: subtract_gradient_program,
        })
    }

    pub fn update(&mut self, settings: &Rc<Settings>) -> () {
        self.settings = Rc::clone(settings); // Fix

        let uniforms = Uniforms {
            timestep: 0.0,
            epsilon: self.grid_size,
            half_epsilon: 0.5 * self.grid_size,
            dissipation: settings.velocity_dissipation,
            texel_size: self.texel_size,
            pad1: 0.0,
            pad2: 0.0,
        };

        unsafe {
            self.context
                .bind_buffer(glow::UNIFORM_BUFFER, Some(self.uniform_buffer.id));
            self.context.buffer_sub_data_u8_slice(
                glow::UNIFORM_BUFFER,
                0,
                &bytemuck::bytes_of(&uniforms),
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
        )?
        .with_data(None::<&[f32]>)?;
        self.velocity_textures
            .blit_to(&self.context, &velocity_textures);
        self.velocity_textures = velocity_textures;

        let divergence_texture = render::Framebuffer::new(
            &self.context,
            width,
            height,
            self.divergence_texture.options,
        )?
        .with_data(None::<&[f32]>)?;
        self.divergence_texture
            .blit_to(&self.context, &divergence_texture);
        self.divergence_texture = divergence_texture;

        let pressure_textures = render::DoubleFramebuffer::new(
            &self.context,
            width,
            height,
            self.pressure_textures.current().options,
        )?
        .with_data(None::<&[f32]>)?;
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

    pub fn advect(&self) -> () {
        self.velocity_textures
            .draw_to(&self.context, |velocity_texture| unsafe {
                self.advection_pass.use_program();

                self.context.active_texture(glow::TEXTURE0);
                self.context
                    .bind_texture(glow::TEXTURE_2D, Some(velocity_texture.texture));
                self.context.active_texture(glow::TEXTURE1);
                self.context
                    .bind_texture(glow::TEXTURE_2D, Some(velocity_texture.texture));

                self.context
                    .draw_elements(glow::TRIANGLES, 6, glow::UNSIGNED_SHORT, 0);
            });
    }

    pub fn diffuse(&self, timestep: f32) -> () {
        let center_factor = self.grid_size.powf(2.0) / (self.settings.viscosity * timestep);
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
                    self.context.active_texture(glow::TEXTURE1);
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
        let alpha = -self.grid_size * self.grid_size;
        let r_beta = 0.25;

        self.pressure_textures
            .clear_color_with(&[self.settings.starting_pressure, 0.0, 0.0, 1.0])
            .unwrap();

        self.pressure_pass.set_uniforms(&[
            &Uniform {
                name: "alpha",
                value: UniformValue::Float(alpha),
            },
            &Uniform {
                name: "rBeta",
                value: UniformValue::Float(r_beta),
            },
        ]);

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
