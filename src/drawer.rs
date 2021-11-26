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
    aspect_ratio: f32,

    grid_width: u32,
    grid_height: u32,
    line_count: u32,
    line_width: f32,
    line_length: f32,

    line_state_textures: render::DoubleFramebuffer,
    basepoint_texture: render::Framebuffer,

    place_lines_pass: render::RenderPass,
    draw_lines_pass: render::RenderPass,
    draw_endpoints_pass: render::RenderPass,
    draw_texture_pass: render::RenderPass,

    view_scale: f32,
    projection_matrix: [f32; 16],
}

impl Drawer {
    pub fn new(
        context: &Context,
        width: u32,
        height: u32,
        grid_width: u32,
        grid_height: u32,
        grid_spacing: u32,
    ) -> Result<Self> {
        let line_count = grid_width * grid_height;
        let aspect_ratio: f32 = (width as f32) / (height as f32);

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
                .with_f32_data(&data::new_line_state(width, height, grid_spacing))?;

        let basepoint_texture =
            render::Framebuffer::new(&context, grid_width, grid_height, texture_options)?
                .with_f32_data(&data::new_points(width, height, grid_spacing))?;

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
                    size: 2,
                    type_: GL::FLOAT,
                    ..Default::default()
                },
            }],
            Indices::NoIndices(GL::TRIANGLES),
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

        let projection_matrix = [
            2.0 / (width as f32),
            0.0,
            0.0,
            0.0,
            0.0,
            -2.0 / (height as f32),
            0.0,
            0.0,
            0.0,
            0.0,
            2.0 / 1.0,
            0.0,
            -1.0,
            1.0,
            0.0,
            1.0,
        ];

        Ok(Self {
            context: context.clone(),
            width,
            height,
            aspect_ratio,
            grid_width,
            grid_height,
            line_count,
            line_width: 10.0,
            line_length: 400.0,

            line_state_textures,
            basepoint_texture,

            place_lines_pass,
            draw_lines_pass,
            draw_endpoints_pass,
            draw_texture_pass,

            view_scale: 1.6,
            projection_matrix,
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
                        name: "uProjection".to_string(),
                        value: UniformValue::Mat4(self.projection_matrix),
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
                        name: "uLineWidth".to_string(),
                        value: UniformValue::Float(self.line_width),
                    },
                    Uniform {
                        name: "uLineLength".to_string(),
                        value: UniformValue::Float(self.line_length),
                    },
                    Uniform {
                        name: "uColor".to_string(),
                        value: UniformValue::Vec3([0.98431373, 0.71764706, 0.19215686]),
                    },
                    Uniform {
                        name: "uViewScale".to_string(),
                        value: UniformValue::Float(self.view_scale),
                    },
                    Uniform {
                        name: "uProjection".to_string(),
                        value: UniformValue::Mat4(self.projection_matrix),
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
                        name: "uLineWidth".to_string(),
                        value: UniformValue::Float(self.line_width),
                    },
                    Uniform {
                        name: "uLineLength".to_string(),
                        value: UniformValue::Float(self.line_length),
                    },
                    Uniform {
                        name: "uColor".to_string(),
                        value: UniformValue::Vec3([0.98431373, 0.71764706, 0.19215686]),
                    },
                    Uniform {
                        name: "uViewScale".to_string(),
                        value: UniformValue::Float(self.view_scale),
                    },
                    Uniform {
                        name: "uProjection".to_string(),
                        value: UniformValue::Mat4(self.projection_matrix),
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

fn get_projection(field_of_view: f32, aspect_ratio: f32, near: f32, far: f32) -> [f32; 16] {
    let f = (field_of_view * 0.5).tan();
    let range_inv = 1.0 / (near - far);
    [
        f / aspect_ratio,
        0.0,
        0.0,
        0.0,
        0.0,
        f,
        0.0,
        0.0,
        0.0,
        0.0,
        (near + far) * range_inv,
        1.0,
        0.0,
        0.0,
        near * far * range_inv * 2.0,
        0.0,
    ]
}
