use crate::{data, render};
use render::{
    BindingInfo, Buffer, Context, DoubleFramebuffer, Framebuffer, Indices, TextureOptions, Uniform,
    UniformValue, VertexBuffer,
};

use std::cell::Ref;

use web_sys::WebGl2RenderingContext as GL;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

static FLUID_VERT_SHADER: &'static str = include_str!("./shaders/fluid.vert");
static ADVECTION_FRAG_SHADER: &'static str = include_str!("./shaders/advection.frag");
static DIVERGENCE_FRAG_SHADER: &'static str = include_str!("./shaders/divergence.frag");
static SOLVE_PRESSURE_FRAG_SHADER: &'static str = include_str!("./shaders/solve_pressure.frag");
static SUBTRACT_GRADIENT_FRAG_SHADER: &'static str =
    include_str!("./shaders/subtract_gradient.frag");
static CURL_FRAG_SHADER: &'static str = include_str!("./shaders/curl.frag");

pub struct Fluid {
    viscosity: f32,
    velocity_dissipation: f32,
    diffusion_iterations: u32,
    pressure_iterations: u32,

    grid_width: u32,
    grid_height: u32,
    texel_size: [f32; 2],
    grid_size: f32,

    velocity_textures: DoubleFramebuffer,
    divergence_texture: Framebuffer,
    pressure_textures: DoubleFramebuffer,

    advection_pass: render::RenderPass,
    diffusion_pass: render::RenderPass,
    divergence_pass: render::RenderPass,
    pressure_pass: render::RenderPass,
    subtract_gradient_pass: render::RenderPass,
    curl_pass: render::RenderPass,
}

impl Fluid {
    pub fn new(
        context: &Context,
        grid_width: u32,
        grid_height: u32,
        viscosity: f32,
        velocity_dissipation: f32,
    ) -> Result<Self> {
        let grid_size: f32 = 1.0;
        let texel_size = [1.0 / grid_width as f32, 1.0 / grid_height as f32];

        let texture_options: TextureOptions = TextureOptions {
            mag_filter: GL::LINEAR,
            min_filter: GL::LINEAR,
            ..Default::default()
        };

        // Framebuffers
        let initial_velocity_data = vec![0.0; (4 * grid_width * grid_height) as usize];
        let velocity_textures =
            render::DoubleFramebuffer::new(&context, grid_width, grid_height, texture_options)?
                .with_f32_data(&initial_velocity_data)?;
        let divergence_texture =
            render::Framebuffer::new(&context, grid_width, grid_height, texture_options)?
                .with_f32_data(&vec![0.0; (grid_width * grid_height * 4) as usize])?;
        let pressure_textures =
            render::DoubleFramebuffer::new(&context, grid_width, grid_height, texture_options)?
                .with_f32_data(&vec![0.0; (grid_width * grid_height * 4) as usize])?;

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
        let divergence_program =
            render::Program::new(&context, (FLUID_VERT_SHADER, DIVERGENCE_FRAG_SHADER))?;
        let pressure_program =
            render::Program::new(&context, (FLUID_VERT_SHADER, SOLVE_PRESSURE_FRAG_SHADER))?;
        let subtract_gradient_program =
            render::Program::new(&context, (FLUID_VERT_SHADER, SUBTRACT_GRADIENT_FRAG_SHADER))?;
        let curl_program = render::Program::new(&context, (FLUID_VERT_SHADER, CURL_FRAG_SHADER))?;

        let advection_pass = render::RenderPass::new(
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
            advection_program,
        )
        .unwrap();
        let diffusion_pass = render::RenderPass::new(
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
            pressure_program.clone(),
        )
        .unwrap();
        let divergence_pass = render::RenderPass::new(
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
            divergence_program,
        )
        .unwrap();
        let pressure_pass = render::RenderPass::new(
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
            pressure_program.clone(),
        )
        .unwrap();
        let subtract_gradient_pass = render::RenderPass::new(
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
            subtract_gradient_program,
        )
        .unwrap();
        let curl_pass = render::RenderPass::new(
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
            curl_program,
        )
        .unwrap();

        Ok(Self {
            viscosity,
            velocity_dissipation,
            diffusion_iterations: 10,
            pressure_iterations: 30,

            grid_width,
            grid_height,
            texel_size,
            grid_size,

            velocity_textures,
            divergence_texture,
            pressure_textures,

            advection_pass,
            diffusion_pass,
            divergence_pass,
            pressure_pass,
            subtract_gradient_pass,
            curl_pass,
        })
    }

    pub fn advect(&self, timestep: f32) -> () {
        self.advection_pass
            .draw_to(
                &self.velocity_textures.next(),
                &vec![
                    Uniform {
                        name: "uTexelSize",
                        value: UniformValue::Vec2(self.texel_size),
                    },
                    Uniform {
                        name: "deltaT",
                        value: UniformValue::Float(timestep),
                    },
                    Uniform {
                        name: "epsilon",
                        value: UniformValue::Float(self.grid_size),
                    },
                    Uniform {
                        name: "dissipation",
                        value: UniformValue::Float(self.velocity_dissipation),
                    },
                    Uniform {
                        name: "inputTexture",
                        value: UniformValue::Texture2D(
                            &self.velocity_textures.current().texture,
                            0,
                        ),
                    },
                    Uniform {
                        name: "velocityTexture",
                        value: UniformValue::Texture2D(
                            &self.velocity_textures.current().texture,
                            1,
                        ),
                    },
                ],
                1,
            )
            .unwrap();

        self.velocity_textures.swap();
    }

    pub fn diffuse(&self, timestep: f32) -> () {
        let center_factor = self.grid_size.powf(2.0) / (self.viscosity * timestep);
        let stencil_factor = 1.0 / (4.0 + center_factor);

        let uniforms = vec![
            Uniform {
                name: "uTexelSize",
                value: UniformValue::Vec2(self.texel_size),
            },
            Uniform {
                name: "alpha",
                value: UniformValue::Float(center_factor),
            },
            Uniform {
                name: "rBeta",
                value: UniformValue::Float(stencil_factor),
            },
        ];

        for uniform in uniforms.into_iter() {
            self.diffusion_pass.set_uniform(&uniform);
        }

        for _ in 0..self.diffusion_iterations {
            self.diffusion_pass
                .draw_to(
                    &self.velocity_textures.next(),
                    &vec![
                        Uniform {
                            name: "divergenceTexture",
                            value: UniformValue::Texture2D(
                                &self.velocity_textures.current().texture,
                                0,
                            ),
                        },
                        Uniform {
                            name: "pressureTexture",
                            value: UniformValue::Texture2D(
                                &self.velocity_textures.current().texture,
                                1,
                            ),
                        },
                    ],
                    1,
                )
                .unwrap();

            self.velocity_textures.swap();
        }
    }

    pub fn calculate_divergence(&self) -> () {
        self.divergence_pass
            .draw_to(
                &self.divergence_texture,
                &vec![
                    Uniform {
                        name: "uTexelSize",
                        value: UniformValue::Vec2(self.texel_size),
                    },
                    Uniform {
                        name: "halfEpsilon",
                        value: UniformValue::Float(0.5 * self.grid_size),
                    },
                    Uniform {
                        name: "velocityTexture",
                        value: UniformValue::Texture2D(
                            &self.velocity_textures.current().texture,
                            0,
                        ),
                    },
                ],
                1,
            )
            .unwrap();
    }

    pub fn solve_pressure(&self) -> () {
        let alpha = -self.grid_size * self.grid_size;
        let r_beta = 0.25;

        self.pressure_textures.zero_out().unwrap();

        let uniforms = vec![
            Uniform {
                name: "uTexelSize",
                value: UniformValue::Vec2(self.texel_size),
            },
            Uniform {
                name: "alpha",
                value: UniformValue::Float(alpha),
            },
            Uniform {
                name: "rBeta",
                value: UniformValue::Float(r_beta),
            },
            Uniform {
                name: "divergenceTexture",
                value: UniformValue::Texture2D(&self.divergence_texture.texture, 0),
            },
        ];

        for uniform in uniforms.into_iter() {
            self.pressure_pass.set_uniform(&uniform);
        }

        for _ in 0..self.pressure_iterations {
            self.pressure_pass
                .draw_to(
                    &self.pressure_textures.next(),
                    &vec![Uniform {
                        name: "pressureTexture",
                        value: UniformValue::Texture2D(
                            &self.pressure_textures.current().texture,
                            1,
                        ),
                    }],
                    1,
                )
                .unwrap();

            self.pressure_textures.swap();
        }
    }

    pub fn subtract_gradient(&self) -> () {
        self.subtract_gradient_pass
            .draw_to(
                &self.velocity_textures.next(),
                &vec![
                    Uniform {
                        name: "uTexelSize",
                        value: UniformValue::Vec2(self.texel_size),
                    },
                    Uniform {
                        name: "halfEpsilon",
                        value: UniformValue::Float(0.5 * self.grid_size),
                    },
                    Uniform {
                        name: "velocityTexture",
                        value: UniformValue::Texture2D(
                            &self.velocity_textures.current().texture,
                            0,
                        ),
                    },
                    Uniform {
                        name: "pressureTexture",
                        value: UniformValue::Texture2D(
                            &self.pressure_textures.current().texture,
                            1,
                        ),
                    },
                ],
                1,
            )
            .unwrap();

        self.velocity_textures.swap();
    }

    pub fn curl(&self, timestep: f32) -> () {
        self.curl_pass
            .draw_to(
                &self.velocity_textures.next(),
                &vec![
                    Uniform {
                        name: "uTexelSize",
                        value: UniformValue::Vec2(self.texel_size),
                    },
                    Uniform {
                        name: "deltaT",
                        value: UniformValue::Float(timestep),
                    },
                    Uniform {
                        name: "velocityTexture",
                        value: UniformValue::Texture2D(
                            &self.velocity_textures.current().texture,
                            0,
                        ),
                    },
                ],
                1,
            )
            .unwrap();

        self.velocity_textures.swap();
    }

    pub fn get_grid_size(&self) -> f32 {
        self.grid_size
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
