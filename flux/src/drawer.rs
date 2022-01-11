use crate::{data, render, settings};
use render::{
    BindingInfo, Buffer, Context, Framebuffer, Indices, Uniform, UniformValue, VertexBuffer,
};
use settings::Settings;

use web_sys::WebGl2RenderingContext as GL;
extern crate nalgebra_glm as glm;
use std::rc::Rc;

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
    settings: Rc<Settings>,

    screen_width: u32,
    screen_height: u32,

    pub grid_width: u32,
    pub grid_height: u32,
    pub line_count: u32,

    // A 6-color hue wheel. Each color gets π/3 or 60° of space.
    color_wheel: [f32; 18],

    line_state_buffers: render::TransformFeedbackBuffer,

    place_lines_pass: render::RenderPass,
    draw_lines_pass: render::RenderPass,
    draw_endpoints_pass: render::RenderPass,
    draw_texture_pass: render::RenderPass,
    antialiasing_pass: render::MsaaPass,

    projection_matrix: glm::TMat4<f32>,
    view_matrix: glm::TMat4<f32>,
}

impl Drawer {
    pub fn update_settings(&mut self, new_settings: &Rc<Settings>) -> () {
        self.settings = new_settings.clone();
        self.color_wheel = settings::color_wheel_from_scheme(&new_settings.color_scheme);
    }

    pub fn resize(&mut self, width: u32, height: u32) -> () {
        let (grid_width, grid_height) = compute_grid_size(width, height);

        self.screen_width = width;
        self.screen_height = height;
        self.grid_width = grid_width;
        self.grid_height = grid_height;

        self.projection_matrix = new_projection_matrix(grid_width, grid_height);
        self.antialiasing_pass.resize(width, height);
    }

    pub fn new(
        context: &Context,
        screen_width: u32,
        screen_height: u32,
        settings: &Rc<Settings>,
    ) -> Result<Self, render::Problem> {
        let (grid_width, grid_height) = compute_grid_size(screen_width, screen_height);

        let line_vertices = Buffer::from_f32(
            &context,
            &data::LINE_VERTICES.to_vec(),
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;

        let basepoint_buffer = Buffer::from_f32(
            &context,
            &data::new_points(grid_width, grid_height, settings.grid_spacing),
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;

        let line_count =
            (grid_width / settings.grid_spacing) * (grid_height / settings.grid_spacing);
        let line_state = data::new_line_state(grid_width, grid_height, settings.grid_spacing);
        let line_state_buffers =
            render::TransformFeedbackBuffer::new_with_f32(&context, &line_state, GL::DYNAMIC_DRAW)?;

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
        )?;
        let plane_indices = Buffer::from_u16(
            &context,
            &data::PLANE_INDICES.to_vec(),
            GL::ELEMENT_ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;

        let place_lines_program = render::Program::new_with_transform_feedback(
            &context,
            (PLACE_LINES_VERT_SHADER, PLACE_LINES_FRAG_SHADER),
            render::TransformFeedback {
                names: vec![
                    "vEndpointVector".to_string(),
                    "vVelocityVector".to_string(),
                    "vLineWidth".to_string(),
                    "vColor".to_string(),
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
            vec![VertexBuffer {
                buffer: basepoint_buffer.clone(),
                binding: BindingInfo {
                    name: "basepoint".to_string(),
                    size: 2,
                    type_: GL::FLOAT,
                    ..Default::default()
                },
            }],
            Indices::NoIndices(GL::POINTS),
            place_lines_program,
        )?;

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
        )?;

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
        )?;

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
        )?;

        let antialiasing_samples = 4;
        let antialiasing_pass =
            render::MsaaPass::new(context, screen_width, screen_height, antialiasing_samples)?;

        let projection_matrix = new_projection_matrix(grid_width, grid_height);

        let view_matrix = glm::scale(
            &glm::identity(),
            &glm::vec3(settings.view_scale, settings.view_scale, 1.0),
        );

        Ok(Self {
            context: context.clone(),
            settings: settings.clone(),

            screen_width,
            screen_height,
            grid_width,
            grid_height,
            line_count,
            color_wheel: settings::color_wheel_from_scheme(&settings.color_scheme),

            line_state_buffers,

            place_lines_pass,
            draw_lines_pass,
            draw_endpoints_pass,
            draw_texture_pass,
            antialiasing_pass,

            projection_matrix,
            view_matrix,
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
                            stride: 8 * 4,
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
                            stride: 8 * 4,
                            offset: 2 * 4,
                            ..Default::default()
                        },
                    },
                    VertexBuffer {
                        buffer: self.line_state_buffers.current().clone(),
                        binding: BindingInfo {
                            name: "iLineWidth".to_string(),
                            size: 1,
                            type_: GL::FLOAT,
                            stride: 8 * 4,
                            offset: 4 * 4,
                            ..Default::default()
                        },
                    },
                ],
                &vec![
                    Uniform {
                        name: "deltaT",
                        value: UniformValue::Float(timestep),
                    },
                    Uniform {
                        name: "uProjection",
                        value: UniformValue::Mat4(self.projection_matrix.as_slice()),
                    },
                    Uniform {
                        name: "uColorWheel[0]",
                        value: UniformValue::Vec3Array(&self.color_wheel),
                    },
                    Uniform {
                        name: "velocityTexture",
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
            .viewport(0, 0, self.screen_width as i32, self.screen_height as i32);

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
                            stride: 8 * 4,
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
                            stride: 8 * 4,
                            offset: 4 * 4,
                            divisor: 1,
                        },
                    },
                    VertexBuffer {
                        buffer: self.line_state_buffers.current().clone(),
                        binding: BindingInfo {
                            name: "iColor".to_string(),
                            size: 3,
                            type_: GL::FLOAT,
                            stride: 8 * 4,
                            offset: 5 * 4,
                            divisor: 1,
                        },
                    },
                ],
                &vec![
                    Uniform {
                        name: "uLineWidth",
                        value: UniformValue::Float(self.settings.line_width),
                    },
                    Uniform {
                        name: "uLineLength",
                        value: UniformValue::Float(self.settings.line_length),
                    },
                    Uniform {
                        name: "uLineBeginOffset",
                        value: UniformValue::Float(self.settings.line_begin_offset),
                    },
                    Uniform {
                        name: "uLineOpacity",
                        value: UniformValue::Float(self.settings.line_opacity),
                    },
                    Uniform {
                        name: "uProjection",
                        value: UniformValue::Mat4(self.projection_matrix.as_slice()),
                    },
                    Uniform {
                        name: "uView",
                        value: UniformValue::Mat4(self.view_matrix.as_slice()),
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
            .viewport(0, 0, self.screen_width as i32, self.screen_height as i32);

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
                            stride: 8 * 4,
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
                            stride: 8 * 4,
                            offset: 4 * 4,
                            divisor: 1,
                        },
                    },
                    VertexBuffer {
                        buffer: self.line_state_buffers.current().clone(),
                        binding: BindingInfo {
                            name: "iColor".to_string(),
                            size: 3,
                            type_: GL::FLOAT,
                            stride: 8 * 4,
                            offset: 5 * 4,
                            divisor: 1,
                        },
                    },
                ],
                &vec![
                    Uniform {
                        name: "uLineWidth",
                        value: UniformValue::Float(self.settings.line_width),
                    },
                    Uniform {
                        name: "uLineLength",
                        value: UniformValue::Float(self.settings.line_length),
                    },
                    Uniform {
                        name: "uLineOpacity",
                        value: UniformValue::Float(self.settings.line_opacity),
                    },
                    Uniform {
                        name: "uProjection",
                        value: UniformValue::Mat4(self.projection_matrix.as_slice()),
                    },
                    Uniform {
                        name: "uView",
                        value: UniformValue::Mat4(self.view_matrix.as_slice()),
                    },
                ],
                None,
                self.line_count,
            )
            .unwrap();

        self.context.disable(GL::BLEND);
    }

    #[allow(dead_code)]
    pub fn draw_texture(&self, texture: &Framebuffer) -> () {
        self.context
            .viewport(0, 0, self.screen_width as i32, self.screen_height as i32);

        self.draw_texture_pass
            .draw(
                &vec![Uniform {
                    name: "inputTexture",
                    value: UniformValue::Texture2D(&texture.texture, 0),
                }],
                1,
            )
            .unwrap();
    }

    pub fn with_antialiasing<T>(&self, draw_call: T) -> ()
    where
        T: Fn() -> (),
    {
        self.antialiasing_pass.draw_to(draw_call);
    }
}

fn compute_grid_size(width: u32, height: u32) -> (u32, u32) {
    let base_units = 1000;
    let aspect_ratio: f32 = (width as f32) / (height as f32);

    // landscape
    if aspect_ratio > 1.0 {
        (base_units, ((base_units as f32) / aspect_ratio) as u32)

    // portrait
    } else {
        (((base_units as f32) * aspect_ratio) as u32, base_units)
    }
}

fn new_projection_matrix(width: u32, height: u32) -> glm::TMat4<f32> {
    let half_width = (width as f32) / 2.0;
    let half_height = (height as f32) / 2.0;

    glm::ortho(
        -half_width,
        half_width,
        -half_height,
        half_height,
        -1.0,
        1.0,
    )
}
