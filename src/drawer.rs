use crate::{data, render};
use render::{
    BindingInfo, Buffer, Context, Framebuffer, Indices, Uniform, UniformValue, VertexBuffer,
};

use web_sys::WebGl2RenderingContext as GL;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

static LINE_VERT_SHADER: &'static str = include_str!("./shaders/line.vert");
static LINE_FRAG_SHADER: &'static str = include_str!("./shaders/line.frag");
static ENDPOINT_VERT_SHADER: &'static str = include_str!("./shaders/endpoint.vert");
static ENDPOINT_FRAG_SHADER: &'static str = include_str!("./shaders/endpoint.frag");
static TEXTURE_VERT_SHADER: &'static str = include_str!("./shaders/texture.vert");
static TEXTURE_FRAG_SHADER: &'static str = include_str!("./shaders/texture.frag");
static PLACE_LINES_VERT_SHADER: &'static str = include_str!("./shaders/place_lines.vert");
static PLACE_LINES_FRAG_SHADER: &'static str = include_str!("./shaders/place_lines.frag");

pub struct Drawer {
    context: Context,

    width: u32,
    height: u32,

    grid_width: u32,
    grid_height: u32,
    line_count: u32,

    line_state_textures: render::DoubleFramebuffer,
    basepoint_texture: render::Framebuffer,

    place_lines_pass: render::RenderPass,
    draw_lines_pass: render::RenderPass,
    draw_endpoints_pass: render::RenderPass,
    draw_texture_pass: render::RenderPass,
}

impl Drawer {
    pub fn new(
        context: &Context,
        width: u32,
        height: u32,
        grid_width: u32,
        grid_height: u32,
    ) -> Result<Self> {
        let line_count = grid_width * grid_height;

        let line_vertices = Buffer::from_f32(
            &context,
            &data::LINE_VERTICES.to_vec(),
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;
        let line_indices = Buffer::from_u16(
            &context,
            &data::LINE_INDICES.to_vec(),
            GL::ELEMENT_ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;

        let circle_vertices = Buffer::from_f32(
            &context,
            &data::new_circle(16),
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;

        // Line state
        //
        // position.x
        // position.y
        // velocity.x
        // velocity.y
        let texture_options: render::TextureOptions = Default::default();
        let line_state_textures =
            render::DoubleFramebuffer::new(&context, grid_width, grid_height, texture_options)?
                .with_f32_data(&data::new_line_state(grid_width as i32, grid_height as i32))?;

        let basepoint_texture =
            render::Framebuffer::new(&context, grid_width, grid_height, texture_options)?
                .with_f32_data(&data::new_points(grid_width as i32, grid_height as i32))?;

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

        let place_lines_program =
            render::Program::new(&context, (PLACE_LINES_VERT_SHADER, PLACE_LINES_FRAG_SHADER))?;
        let draw_lines_program =
            render::Program::new(&context, (LINE_VERT_SHADER, LINE_FRAG_SHADER))?;
        let draw_endpoints_program =
            render::Program::new(&context, (ENDPOINT_VERT_SHADER, ENDPOINT_FRAG_SHADER))?;
        let draw_texture_program =
            render::Program::new(&context, (TEXTURE_VERT_SHADER, TEXTURE_FRAG_SHADER))?;

        let place_lines_pass = render::RenderPass::new(
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
            place_lines_program,
        )
        .unwrap();

        let draw_lines_pass = render::RenderPass::new(
            &context,
            vec![VertexBuffer {
                buffer: line_vertices.clone(),
                binding: BindingInfo {
                    name: "vertex".to_string(),
                    size: 3,
                    type_: GL::FLOAT,
                    ..Default::default()
                },
            }],
            Indices::IndexBuffer {
                buffer: line_indices,
                primitive: GL::TRIANGLES,
            },
            draw_lines_program,
        )
        .unwrap();

        let draw_endpoints_pass = render::RenderPass::new(
            &context,
            vec![VertexBuffer {
                buffer: circle_vertices.clone(),
                binding: BindingInfo {
                    name: "vertex".to_string(),
                    size: 2,
                    type_: GL::FLOAT,
                    ..Default::default()
                },
            }],
            Indices::NoIndices(GL::TRIANGLE_FAN),
            draw_endpoints_program,
        )
        .unwrap();

        let draw_texture_pass = render::RenderPass::new(
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
            draw_texture_program,
        )
        .unwrap();

        Ok(Self {
            context: context.clone(),
            width,
            height,
            grid_width: grid_width,
            grid_height: grid_height,
            line_count: line_count,

            line_state_textures,
            basepoint_texture,

            place_lines_pass,
            draw_lines_pass,
            draw_endpoints_pass,
            draw_texture_pass,
        })
    }

    pub fn place_lines(&self, timestep: f32, texture: &Framebuffer) -> () {
        self.place_lines_pass
            .draw_to(
                &self.line_state_textures.next(),
                vec![
                    Uniform {
                        name: "deltaT".to_string(),
                        value: UniformValue::Float(timestep),
                    },
                    Uniform {
                        name: "lineCount".to_string(),
                        value: UniformValue::UnsignedInt(self.line_count),
                    },
                    Uniform {
                        name: "basepointTexture".to_string(),
                        value: UniformValue::Texture2D(&self.basepoint_texture.texture, 0),
                    },
                    Uniform {
                        name: "lineStateTexture".to_string(),
                        value: UniformValue::Texture2D(
                            &self.line_state_textures.current().texture,
                            1,
                        ),
                    },
                    Uniform {
                        name: "velocityTexture".to_string(),
                        value: UniformValue::Texture2D(&texture.texture, 2),
                    },
                ],
                1,
            )
            .unwrap();

        self.line_state_textures.swap();
    }

    pub fn draw_lines(&self, timestep: f32) -> () {
        self.context
            .viewport(0, 0, self.width as i32, self.height as i32);

        self.context.enable(GL::BLEND);
        self.context.blend_func(GL::SRC_ALPHA, GL::ONE);

        self.draw_lines_pass
            .draw(
                vec![
                    Uniform {
                        name: "deltaT".to_string(),
                        value: UniformValue::Float(timestep),
                    },
                    Uniform {
                        name: "lineCount".to_string(),
                        value: UniformValue::UnsignedInt(self.line_count),
                    },
                    Uniform {
                        name: "uColor".to_string(),
                        value: UniformValue::Vec3([0.98431373, 0.71764706, 0.19215686]),
                    },
                    Uniform {
                        name: "lineStateTexture".to_string(),
                        value: UniformValue::Texture2D(
                            &self.line_state_textures.current().texture,
                            0,
                        ),
                    },
                ],
                self.line_count,
            )
            .unwrap();

        self.context.disable(GL::BLEND);
    }

    pub fn draw_endpoints(&self) -> () {
        self.context
            .viewport(0, 0, self.width as i32, self.height as i32);

        self.context.enable(GL::BLEND);
        self.context.blend_func(GL::SRC_ALPHA, GL::ONE);

        self.draw_endpoints_pass
            .draw(
                vec![
                    Uniform {
                        name: "uColor".to_string(),
                        value: UniformValue::Vec3([0.98431373, 0.71764706, 0.19215686]),
                    },
                    Uniform {
                        name: "lineStateTexture".to_string(),
                        value: UniformValue::Texture2D(
                            &self.line_state_textures.current().texture,
                            0,
                        ),
                    },
                ],
                self.line_count,
            )
            .unwrap();

        self.context.disable(GL::BLEND);
    }

    pub fn draw_texture(&self, texture: &Framebuffer) -> () {
        self.context
            .viewport(0, 0, self.width as i32, self.height as i32);

        self.draw_texture_pass
            .draw(
                vec![Uniform {
                    name: "inputTexture".to_string(),
                    value: UniformValue::Texture2D(&texture.texture, 0),
                }],
                1,
            )
            .unwrap();
    }
}
