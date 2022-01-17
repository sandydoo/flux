use crate::{data, render, settings};
use render::{
    Buffer, Context, DoubleFramebuffer, Framebuffer, Indices, TextureOptions, Uniform, UniformValue,
};
use settings::Settings;

use bytemuck::{Pod, Zeroable};
use std::cell::Ref;
use std::f32::consts::PI;
use std::rc::Rc;

use web_sys::WebGl2RenderingContext as GL;
use web_sys::WebGlVertexArrayObject;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

static FLUID_VERT_SHADER: &'static str = include_str!("./shaders/fluid.vert");
static ADVECTION_FRAG_SHADER: &'static str = include_str!("./shaders/advection.frag");
static DIVERGENCE_FRAG_SHADER: &'static str = include_str!("./shaders/divergence.frag");
static SOLVE_PRESSURE_FRAG_SHADER: &'static str = include_str!("./shaders/solve_pressure.frag");
static SUBTRACT_GRADIENT_FRAG_SHADER: &'static str =
    include_str!("./shaders/subtract_gradient.frag");

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    timestep: f32,
    pad1: f32,
    texel_size: [f32; 2],
    epsilon: f32,
    half_epsilon: f32,
    dissipation: f32,
    padding: f32,
}

pub struct Fluid {
    context: Context,
    settings: Rc<Settings>,

    texel_size: [f32; 2],
    grid_size: f32,

    plane_indices: Buffer,
    uniform_buffer: Buffer,
    fluid_vertex_buffer: WebGlVertexArrayObject,

    velocity_textures: DoubleFramebuffer,
    divergence_texture: Framebuffer,
    pressure_textures: DoubleFramebuffer,

    advection_pass: render::Program,
    // diffusion_pass: render::Program,
    divergence_pass: render::Program,
    pressure_pass: render::Program,
    subtract_gradient_pass: render::Program,
}

impl Fluid {
    pub fn update_settings(&mut self, new_settings: &Rc<Settings>) -> () {
        self.settings = new_settings.clone();
    }

    pub fn new(context: &Context, settings: &Rc<Settings>) -> Result<Self> {
        let grid_size: f32 = 1.0;
        let grid_width = settings.fluid_width;
        let grid_height = settings.fluid_height;
        let texel_size = [1.0 / grid_width as f32, 1.0 / grid_height as f32];

        // Framebuffers
        // let initial_velocity_data = vec![0.2; (2 * grid_width * grid_height) as usize];
        let initial_velocity_data =
            data::make_sine_vector_field(grid_width as i32, grid_height as i32);
        // let mut initial_velocity_data = Vec::with_capacity((grid_width * grid_height * 2) as usize);
        // for v in 0..grid_height {
        //     for u in 0..grid_width {
        //         let r = PI * (u as f32) / (grid_width as f32);

        //         // let r = (u as f32) / (grid_width as f32);
        //         // super::log!("{}", r.cos());
        //         initial_velocity_data.push(0.2 * r.sin());
        //         initial_velocity_data.push(0.2 * r.cos());
        //         // initial_velocity_data.push(-1.0);
        //         // initial_velocity_data.push(-1.0);
        //         // initial_velocity_data.push(u as f32);
        //         // initial_velocity_data.push(v as f32);
        //     }
        // }
        let velocity_textures = render::DoubleFramebuffer::new(
            &context,
            grid_width,
            grid_height,
            TextureOptions {
                mag_filter: GL::LINEAR,
                min_filter: GL::LINEAR,
                format: GL::RG32F,
                ..Default::default()
            },
        )?
        .with_f32_data(&initial_velocity_data)?;

        let divergence_texture = render::Framebuffer::new(
            &context,
            grid_width,
            grid_height,
            TextureOptions {
                mag_filter: GL::LINEAR,
                min_filter: GL::LINEAR,
                format: GL::RG32F,
                ..Default::default()
            },
        )?
        .with_f32_data(&vec![0.0; (2 * grid_width * grid_height) as usize])?;

        let pressure_textures = render::DoubleFramebuffer::new(
            &context,
            grid_width,
            grid_height,
            TextureOptions {
                mag_filter: GL::LINEAR,
                min_filter: GL::LINEAR,
                format: GL::RG32F,
                ..Default::default()
            },
        )?
        .with_f32_data(&vec![0.0; (2 * grid_width * grid_height) as usize])?;

        // Geometry
        let plane_vertices = Buffer::from_f32(
            &context,
            &data::PLANE_VERTICES.to_vec(),
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;
        let plane_indices = Buffer::from_u16(
            &context,
            &data::PLANE_INDICES.to_vec(),
            GL::ELEMENT_ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;

        let advection_program =
            render::Program::new(&context, (FLUID_VERT_SHADER, ADVECTION_FRAG_SHADER))?;
        let divergence_program =
            render::Program::new(&context, (FLUID_VERT_SHADER, DIVERGENCE_FRAG_SHADER))?;
        let pressure_program =
            render::Program::new(&context, (FLUID_VERT_SHADER, SOLVE_PRESSURE_FRAG_SHADER))?;
        let subtract_gradient_program =
            render::Program::new(&context, (FLUID_VERT_SHADER, SUBTRACT_GRADIENT_FRAG_SHADER))?;

        let uniforms = Uniforms {
            timestep: 0.0,
            pad1: 0.0,
            texel_size,
            epsilon: grid_size,
            half_epsilon: 0.5 * grid_size,
            dissipation: settings.velocity_dissipation,
            padding: 0.0,
        };

        let uniform_buffer = Buffer::from_f32_array(
            &context,
            &bytemuck::cast_slice(&[uniforms]),
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;

        let uniforms_block_index =
            context.get_uniform_block_index(&advection_program.program, "Uniforms");
        super::log!("setting {}", "Uniforms");
        context.uniform_block_binding(&advection_program.program, uniforms_block_index, 0);
        let uniforms_block_index =
            context.get_uniform_block_index(&divergence_program.program, "Uniforms");
        super::log!("setting {}", "Uniforms");
        context.uniform_block_binding(&divergence_program.program, uniforms_block_index, 0);
        let uniforms_block_index =
            context.get_uniform_block_index(&pressure_program.program, "Uniforms");
        super::log!("setting {}", "Uniforms");
        context.uniform_block_binding(&pressure_program.program, uniforms_block_index, 0);
        let uniforms_block_index =
            context.get_uniform_block_index(&subtract_gradient_program.program, "Uniforms");
        super::log!("setting {}", "Uniforms");
        context.uniform_block_binding(&subtract_gradient_program.program, uniforms_block_index, 0);

        let fluid_vertex_buffer = render::create_vertex_array(
            &context,
            &advection_program,
            &[(
                &plane_vertices,
                render::VertexBufferLayout {
                    name: "position",
                    size: 3,
                    type_: GL::FLOAT,
                    ..Default::default()
                },
            )],
        );
        context.bind_vertex_array(Some(&fluid_vertex_buffer));
        context.bind_buffer(GL::ELEMENT_ARRAY_BUFFER, Some(&plane_indices.id));
        context.bind_vertex_array(None);

        Ok(Self {
            context: Rc::clone(context),
            settings: Rc::clone(settings),

            texel_size,
            grid_size,

            plane_indices,
            uniform_buffer,
            fluid_vertex_buffer,

            velocity_textures,
            divergence_texture,
            pressure_textures,

            advection_pass: advection_program,
            // diffusion_pass: pressure_program.clone(),
            divergence_pass: divergence_program,
            pressure_pass: pressure_program,
            subtract_gradient_pass: subtract_gradient_program,
        })
    }

    pub fn advect(&self, timestep: f32) -> () {
        self.context
            .bind_buffer(GL::UNIFORM_BUFFER, Some(&self.uniform_buffer.id));
        self.context
            .buffer_sub_data_with_i32_and_u8_array_and_src_offset_and_length(
                GL::UNIFORM_BUFFER,
                0 * 4,
                &timestep.to_ne_bytes(),
                0,
                4,
            );
        self.context.bind_buffer(GL::UNIFORM_BUFFER, None);

        draw_to(&self.context, &self.velocity_textures.next(), || {
            self.context.use_program(Some(&self.advection_pass.program));

            self.context
                .bind_vertex_array(Some(&self.fluid_vertex_buffer));

            self.context.active_texture(GL::TEXTURE0);
            self.context.bind_texture(
                GL::TEXTURE_2D,
                Some(&self.velocity_textures.current().texture),
            );
            self.context.active_texture(GL::TEXTURE1);
            self.context.bind_texture(
                GL::TEXTURE_2D,
                Some(&self.velocity_textures.current().texture),
            );

            self.context
                .bind_buffer_base(GL::UNIFORM_BUFFER, 0, Some(&self.uniform_buffer.id));

            self.context
                .draw_elements_with_i32(GL::TRIANGLES, 6, GL::UNSIGNED_SHORT, 0);
        });

        self.velocity_textures.swap();
    }

    // pub fn diffuse(&self, timestep: f32) -> () {
    //     let center_factor = self.grid_size.powf(2.0) / (self.settings.viscosity * timestep);
    //     let stencil_factor = 1.0 / (4.0 + center_factor);

    //     let uniforms = [
    //         Uniform {
    //             name: "uTexelSize",
    //             value: UniformValue::Vec2(self.texel_size),
    //         },
    //         Uniform {
    //             name: "alpha",
    //             value: UniformValue::Float(center_factor),
    //         },
    //         Uniform {
    //             name: "rBeta",
    //             value: UniformValue::Float(stencil_factor),
    //         },
    //     ];

    //     for uniform in uniforms.into_iter() {
    //         self.diffusion_pass.set_uniform(&uniform);
    //     }

    //     for _ in 0..self.settings.diffusion_iterations {
    //         self.diffusion_pass
    //             .draw_to(
    //                 &self.velocity_textures.next(),
    //                 &[
    //                     Uniform {
    //                         name: "divergenceTexture",
    //                         value: UniformValue::Texture2D(
    //                             &self.velocity_textures.current().texture,
    //                             0,
    //                         ),
    //                     },
    //                     Uniform {
    //                         name: "pressureTexture",
    //                         value: UniformValue::Texture2D(
    //                             &self.velocity_textures.current().texture,
    //                             1,
    //                         ),
    //                     },
    //                 ],
    //                 1,
    //             )
    //             .unwrap();

    //         self.velocity_textures.swap();
    //     }
    // }

    pub fn calculate_divergence(&self) -> () {
        draw_to(&self.context, &self.divergence_texture, || {
            self.context
                .use_program(Some(&self.divergence_pass.program));

            // self.context
            //     .bind_vertex_array(Some(&self.fluid_vertex_buffer));

            self.context.active_texture(GL::TEXTURE0);
            self.context.bind_texture(
                GL::TEXTURE_2D,
                Some(&self.velocity_textures.current().texture),
            );

            // self.context
            //     .bind_buffer_base(GL::UNIFORM_BUFFER, 0, Some(&self.uniform_buffer.id));

            self.context
                .draw_elements_with_i32(GL::TRIANGLES, 6, GL::UNSIGNED_SHORT, 0);
        });
    }

    pub fn solve_pressure(&self) -> () {
        let alpha = -self.grid_size * self.grid_size;
        let r_beta = 0.25;

        self.pressure_textures.zero_out().unwrap();

        self.context.use_program(Some(&self.pressure_pass.program));

        self.context.uniform1f(
            self.pressure_pass.get_uniform_location("alpha").as_ref(),
            alpha,
        );
        self.context.uniform1f(
            self.pressure_pass.get_uniform_location("rBeta").as_ref(),
            r_beta,
        );

        self.context.active_texture(GL::TEXTURE0);
        self.context
            .bind_texture(GL::TEXTURE_2D, Some(&self.divergence_texture.texture));

        // self.context
        //     .bind_buffer_base(GL::UNIFORM_BUFFER, 0, Some(&self.uniform_buffer.id));

        for _ in 0..self.settings.pressure_iterations {
            self.context.active_texture(GL::TEXTURE1);
            self.context.bind_texture(
                GL::TEXTURE_2D,
                Some(&self.pressure_textures.current().texture),
            );

            draw_to(&self.context, &self.pressure_textures.next(), || {
                self.context
                    .draw_elements_with_i32(GL::TRIANGLES, 6, GL::UNSIGNED_SHORT, 0);
            });

            self.pressure_textures.swap();
        }
    }

    pub fn subtract_gradient(&self) -> () {
        self.context
            .use_program(Some(&self.subtract_gradient_pass.program));

        draw_to(&self.context, &self.velocity_textures.next(), || {
            self.context.active_texture(GL::TEXTURE0);
            self.context.bind_texture(
                GL::TEXTURE_2D,
                Some(&self.velocity_textures.current().texture),
            );
            self.context.active_texture(GL::TEXTURE1);
            self.context.bind_texture(
                GL::TEXTURE_2D,
                Some(&self.pressure_textures.current().texture),
            );
            // self.context
            //     .bind_buffer_base(GL::UNIFORM_BUFFER, 0, Some(&self.uniform_buffer.id));

            self.context
                .draw_elements_with_i32(GL::TRIANGLES, 6, GL::UNSIGNED_SHORT, 0);
        });

        self.velocity_textures.swap();
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

pub fn draw_to<T>(context: &Context, framebuffer: &Framebuffer, draw: T) -> Result<()>
where
    T: Fn() -> (),
{
    context.bind_framebuffer(GL::DRAW_FRAMEBUFFER, Some(&framebuffer.id));
    context.viewport(0, 0, framebuffer.width as i32, framebuffer.height as i32);

    draw();

    context.bind_framebuffer(GL::DRAW_FRAMEBUFFER, None);

    Ok(())
}
