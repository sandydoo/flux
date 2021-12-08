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
    line_begin_offset: f32,
    color: [f32; 3],

    line_state_buffers: render::TransformFeedbackBuffer,
    line_index_buffer: render::Buffer,
    basepoint_buffer: render::Buffer,

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

        let basepoint_buffer = Buffer::from_f32(
            &context,
            &data::new_points(width, height, grid_spacing),
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;

        let mut line_indices = Vec::with_capacity(line_count as usize);
        for i in 0..line_count {
            line_indices.push(i as u16);
        }
        let line_index_buffer =
            Buffer::from_u16(&context, &line_indices, GL::ARRAY_BUFFER, GL::STATIC_DRAW)?;

        let line_state = data::new_line_state(width, height, grid_spacing);
        let line_state_buffers =
            render::TransformFeedbackBuffer::new_with_f32(&context, &line_state, GL::STATIC_DRAW)?;

        let circle_vertices = Buffer::from_f32(
            &context,
            &data::new_semicircle(8),
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;

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

        let place_lines_program = render::Program::new_with_transform_feedback(
            &context,
            (PLACE_LINES_VERT_SHADER, PLACE_LINES_FRAG_SHADER),
            render::TransformFeedback {
                names: vec![
                    "vEndpointVector".to_string(),
                    "vVelocityVector".to_string(),
                    "vLineWidth".to_string(),
                ],
                mode: GL::INTERLEAVED_ATTRIBS,
            },
        )?;
        let draw_lines_program =
            render::Program::new(&context, (LINE_VERT_SHADER, LINE_FRAG_SHADER))?;
        let draw_endpoints_program =
            render::Program::new(&context, (ENDPOINT_VERT_SHADER, ENDPOINT_FRAG_SHADER))?;
        let draw_texture_program =
            render::Program::new(&context, (TEXTURE_VERT_SHADER, TEXTURE_FRAG_SHADER))?;

        let place_lines_pass = render::RenderPass::new(
            &context,
            vec![
                VertexBuffer {
                    buffer: basepoint_buffer.clone(),
                    binding: BindingInfo {
                        name: "basepoint".to_string(),
                        size: 2,
                        type_: GL::FLOAT,
                        ..Default::default()
                    },
                },
                // VertexBuffer {
                //     buffer: line_index_buffer.clone(),
                //     binding: BindingInfo {
                //         name: "lineIndex".to_string(),
                //         size: 1,
                //         type_: GL::UNSIGNED_SHORT,
                //         ..Default::default()
                //     },
                // },
            ],
            Indices::NoIndices(GL::POINTS),
            place_lines_program,
        )
        .unwrap();

        let draw_lines_pass = render::RenderPass::new(
            &context,
            vec![
                VertexBuffer {
                    buffer: line_vertices.clone(),
                    binding: BindingInfo {
                        name: "lineVertex".to_string(),
                        size: 2,
                        type_: GL::FLOAT,
                        ..Default::default()
                    },
                },
                VertexBuffer {
                    buffer: basepoint_buffer.clone(),
                    binding: BindingInfo {
                        name: "basepoint".to_string(),
                        size: 2,
                        type_: GL::FLOAT,
                        divisor: 1,
                        ..Default::default()
                    },
                },
            ],
            Indices::NoIndices(GL::TRIANGLES),
            draw_lines_program,
        )
        .unwrap();

        let draw_endpoints_pass = render::RenderPass::new(
            &context,
            vec![
                VertexBuffer {
                    buffer: circle_vertices.clone(),
                    binding: BindingInfo {
                        name: "vertex".to_string(),
                        size: 2,
                        type_: GL::FLOAT,
                        ..Default::default()
                    },
                },
                VertexBuffer {
                    buffer: basepoint_buffer.clone(),
                    binding: BindingInfo {
                        name: "basepoint".to_string(),
                        size: 2,
                        type_: GL::FLOAT,
                        divisor: 1,
                        ..Default::default()
                    },
                },
            ],
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
            2.0 / (height as f32),
            0.0,
            0.0,
            0.0,
            0.0,
            2.0 / 1.0,
            0.0,
            0.0,
            0.0,
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
            line_length: 300.0,
            line_begin_offset: 0.4,
            // pink
            // 0.99215686, 0.67058824, 0.57254902
            // yellow
            // 0.98431373, 0.71764706, 0.19215686
            color: [0.99215686, 0.67058824, 0.57254902],

            line_state_buffers,
            line_index_buffer,
            basepoint_buffer,

            place_lines_pass,
            draw_lines_pass,
            draw_endpoints_pass,
            draw_texture_pass,

            view_scale: 1.4,
            projection_matrix,
        })
    }

    pub fn place_lines(&self, timestep: f32, texture: &Framebuffer) -> () {
        self.place_lines_pass
            .draw_impl(
                vec![
                    VertexBuffer {
                        buffer: self.line_state_buffers.current().clone(),
                        binding: BindingInfo {
                            name: "iEndpointVector".to_string(),
                            size: 2,
                            type_: GL::FLOAT,
                            stride: 5 * 4,
                            offset: 0 * 4,
                            ..Default::default()
                        },
                    },
                    VertexBuffer {
                        buffer: self.line_state_buffers.current().clone(),
                        binding: BindingInfo {
                            name: "iVelocityVector".to_string(),
                            size: 2,
                            type_: GL::FLOAT,
                            stride: 5 * 4,
                            offset: 2 * 4,
                            ..Default::default()
                        },
                    },
                    // VertexBuffer {
                    //     buffer: self.line_state_buffers.current().clone(),
                    //     binding: BindingInfo {
                    //         name: "iLineWidth".to_string(),
                    //         size: 1,
                    //         type_: GL::FLOAT,
                    //         stride: 5 * 4,
                    //         offset: 4 * 4,
                    //         ..Default::default()
                    //     },
                    // },
                ],
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
                        name: "velocityTexture".to_string(),
                        value: UniformValue::Texture2D(&texture.texture, 0),
                    },
                ],
                Some(&self.line_state_buffers),
                1,
            )
            .unwrap();

        self.line_state_buffers.swap();
    }

    pub fn draw_lines(&self) -> () {
        self.context
            .viewport(0, 0, self.width as i32, self.height as i32);

        self.context.enable(GL::BLEND);
        self.context.blend_func(GL::SRC_ALPHA, GL::ONE);

        self.draw_lines_pass
            .draw_impl(
                vec![
                    VertexBuffer {
                        buffer: self.line_state_buffers.current().clone(),
                        binding: BindingInfo {
                            name: "iEndpointVector".to_string(),
                            size: 2,
                            type_: GL::FLOAT,
                            stride: 5 * 4,
                            offset: 0 * 4,
                            divisor: 1,
                        },
                    },
                    VertexBuffer {
                        buffer: self.line_state_buffers.current().clone(),
                        binding: BindingInfo {
                            name: "iLineWidth".to_string(),
                            size: 1,
                            type_: GL::FLOAT,
                            stride: 5 * 4,
                            offset: 4 * 4,
                            divisor: 1,
                        },
                    },
                ],
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
                        name: "uLineBeginOffset".to_string(),
                        value: UniformValue::Float(self.line_begin_offset),
                    },
                    Uniform {
                        name: "uColor".to_string(),
                        value: UniformValue::Vec3(self.color),
                    },
                    Uniform {
                        name: "uViewScale".to_string(),
                        value: UniformValue::Float(self.view_scale),
                    },
                    Uniform {
                        name: "uProjection".to_string(),
                        value: UniformValue::Mat4(self.projection_matrix),
                    },
                ],
                None,
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
            .draw_impl(
                vec![
                    VertexBuffer {
                        buffer: self.line_state_buffers.current().clone(),
                        binding: BindingInfo {
                            name: "iEndpointVector".to_string(),
                            size: 2,
                            type_: GL::FLOAT,
                            stride: 5 * 4,
                            offset: 0 * 4,
                            divisor: 1,
                        },
                    },
                    VertexBuffer {
                        buffer: self.line_state_buffers.current().clone(),
                        binding: BindingInfo {
                            name: "iLineWidth".to_string(),
                            size: 1,
                            type_: GL::FLOAT,
                            stride: 5 * 4,
                            offset: 4 * 4,
                            divisor: 1,
                        },
                    },
                ],
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
                        value: UniformValue::Vec3(self.color),
                    },
                    Uniform {
                        name: "uViewScale".to_string(),
                        value: UniformValue::Float(self.view_scale),
                    },
                    Uniform {
                        name: "uProjection".to_string(),
                        value: UniformValue::Mat4(self.projection_matrix),
                    },
                ],
                None,
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
