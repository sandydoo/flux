use crate::{data, render};
use render::{
    BindingInfo, Buffer, Context, DoubleFramebuffer, Indices, TextureOptions, Uniform,
    UniformValue, VertexBuffer,
};

use web_sys::WebGl2RenderingContext as GL;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

static FLUID_VERT_SHADER: &'static str = include_str!("./shaders/textured_quad.vert");
static ADVECTION_FRAG_SHADER: &'static str = include_str!("./shaders/advection.frag");

pub struct Fluid {
    viscosity: f32,
    velocity_dissipation: f32,

    grid_width: u32,
    grid_height: u32,

    velocity_textures: DoubleFramebuffer,

    advection_pass: render::RenderPass,
}

impl Fluid {
    pub fn new(
        context: &Context,
        grid_width: u32,
        grid_height: u32,
        viscosity: f32,
        velocity_dissipation: f32,
    ) -> Result<Self> {
        let texture_options: TextureOptions = Default::default();

        // Framebuffers
        let initial_velocity_data =
            data::make_sine_vector_field(grid_width as i32, grid_height as i32);
        let velocity_textures =
            render::DoubleFramebuffer::new(&context, grid_width, grid_height, texture_options)?
                .with_f32_data(&initial_velocity_data)?;

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

        let advection_program =
            render::Program::new(&context, (FLUID_VERT_SHADER, ADVECTION_FRAG_SHADER))?;

        let advection_pass = render::RenderPass::new(
            &context,
            vec![VertexBuffer {
                buffer: plane_vertices,
                binding: BindingInfo {
                    name: "position".to_string(),
                    size: 3,
                    type_: GL::FLOAT,
                    ..Default::default()
                },
            }],
            Indices::IndexBuffer {
                buffer: plane_indices,
                primitive: GL::TRIANGLES,
            },
            advection_program,
        )
        .unwrap();

        Ok(Self {
            viscosity,
            velocity_dissipation,
            grid_width,
            grid_height,

            velocity_textures,
            advection_pass,
        })
    }

    pub fn advect(&self, timestep: f32) -> () {
        // TODO: fix
        let epsilon: f32 = 0.0001;

        self.advection_pass
            .draw_to(
                &self.velocity_textures.next(),
                vec![
                    Uniform {
                        name: "uTexelSize".to_string(),
                        value: UniformValue::Float(epsilon),
                    },
                    Uniform {
                        name: "deltaT".to_string(),
                        value: UniformValue::Float(timestep),
                    },
                    Uniform {
                        name: "epsilon".to_string(),
                        value: UniformValue::Float(epsilon),
                    },
                    Uniform {
                        name: "dissipation".to_string(),
                        value: UniformValue::Float(self.velocity_dissipation),
                    },
                    Uniform {
                        name: "inputTexture".to_string(),
                        value: UniformValue::Texture2D(
                            &self.velocity_textures.current().texture,
                            0,
                        ),
                    },
                    Uniform {
                        name: "velocityTexture".to_string(),
                        value: UniformValue::Texture2D(
                            &self.velocity_textures.current().texture,
                            1,
                        ),
                    },
                ],
            )
            .unwrap();

        self.velocity_textures.swap();
    }
}
