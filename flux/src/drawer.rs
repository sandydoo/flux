use crate::{data, render, settings};
use render::{
    Buffer, Context, Framebuffer, Uniform, UniformValue, VertexArrayObject, VertexBufferLayout,
};
use settings::Settings;

use web_sys::{WebGl2RenderingContext as GL, WebGlTransformFeedback};
extern crate nalgebra_glm as glm;
use bytemuck::{Pod, Zeroable};
use std::f32::consts::PI;
use std::rc::Rc;

static LINE_VERT_SHADER: &'static str = include_str!("./shaders/line.vert");
static LINE_FRAG_SHADER: &'static str = include_str!("./shaders/line.frag");
static ENDPOINT_VERT_SHADER: &'static str = include_str!("./shaders/endpoint.vert");
static ENDPOINT_FRAG_SHADER: &'static str = include_str!("./shaders/endpoint.frag");
static TEXTURE_VERT_SHADER: &'static str = include_str!("./shaders/texture.vert");
static TEXTURE_FRAG_SHADER: &'static str = include_str!("./shaders/texture.frag");
static PLACE_LINES_VERT_SHADER: &'static str = include_str!("./shaders/place_lines.vert");
static PLACE_LINES_FRAG_SHADER: &'static str = include_str!("./shaders/place_lines.frag");

#[rustfmt::skip]
const LINE_VERTICES: [f32; 12] = [
    0.0, -0.5,
    1.0, -0.5,
    1.0, 0.5,
    0.0, -0.5,
    1.0, 0.5,
    0.0, 0.5,
];

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LineState {
    endpoint: [f32; 2],
    velocity: [f32; 2],
    color: [f32; 4],
    width: f32,
    opacity: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Projection {
    projection: [f32; 16],
    view: [f32; 16],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LineUniforms {
    line_width: f32,
    line_length: f32,
    line_begin_offset: f32,
    line_fade_out_length: f32,
}

impl LineUniforms {
    fn new(settings: &Rc<Settings>, timestep: f32) -> Self {
        Self {
            line_width: settings.line_width,
            line_length: settings.line_length,
            line_begin_offset: settings.line_begin_offset,
            line_fade_out_length: settings.line_fade_out_length,
        }
    }
}

pub struct Drawer {
    context: Context,

    screen_width: u32,
    screen_height: u32,

    pub grid_width: u32,
    pub grid_height: u32,
    pub grid_spacing: u32,
    pub line_count: u32,

    basepoint_buffer: Buffer,
    line_state_buffer: Buffer,
    transform_feedback_buffer: WebGlTransformFeedback,
    // A dedicated buffer to write out the data from the transform feedback pass
    line_state_feedback_buffer: Buffer,

    place_lines_buffer: VertexArrayObject,
    draw_lines_buffer: VertexArrayObject,
    draw_endpoints_buffer: VertexArrayObject,
    draw_texture_buffer: VertexArrayObject,

    view_buffer: Buffer,
    line_uniforms: Buffer,

    place_lines_pass: render::Program,
    draw_lines_pass: render::Program,
    draw_endpoints_pass: render::Program,
    draw_texture_pass: render::Program,
    antialiasing_pass: render::MsaaPass,
}

impl Drawer {
    pub fn new(
        context: &Context,
        screen_width: u32,
        screen_height: u32,
        settings: &Rc<Settings>,
    ) -> Result<Self, render::Problem> {
        let (grid_width, grid_height) = (screen_width, screen_height);

        let line_count =
            (grid_width / settings.grid_spacing) * (grid_height / settings.grid_spacing);
        let line_state = new_line_state(grid_width, grid_height, settings.grid_spacing);
        let line_state_buffer = Buffer::from_f32(
            &context,
            &bytemuck::cast_slice(&line_state),
            GL::ARRAY_BUFFER,
            GL::DYNAMIC_COPY,
        )?;
        let transform_feedback_buffer = context
            .create_transform_feedback()
            .ok_or(render::Problem::OutOfMemory)?;

        let line_vertices = Buffer::from_f32(
            &context,
            &bytemuck::cast_slice(&LINE_VERTICES),
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;
        let basepoint_buffer = Buffer::from_f32(
            &context,
            &new_basepoints(grid_width, grid_height, settings.grid_spacing),
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;
        let endpoint_vertices = Buffer::from_f32(
            &context,
            &new_endpoint(16),
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;
        let plane_vertices = Buffer::from_f32(
            &context,
            &data::PLANE_VERTICES,
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;
        let plane_indices = Buffer::from_u16(
            &context,
            &data::PLANE_INDICES,
            GL::ELEMENT_ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;

        // Programs

        let place_lines_program = render::Program::new_with_transform_feedback(
            &context,
            (PLACE_LINES_VERT_SHADER, PLACE_LINES_FRAG_SHADER),
            &render::TransformFeedback {
                // The order here must match the order in the buffer!
                names: &[
                    "vEndpointVector",
                    "vVelocityVector",
                    "vColor",
                    "vLineWidth",
                    "vOpacity",
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

        // Vertex buffers

        let place_lines_buffer = VertexArrayObject::empty(context)?;
        let draw_lines_buffer = VertexArrayObject::new(
            context,
            &draw_lines_program,
            &[(
                &line_vertices,
                VertexBufferLayout {
                    name: "lineVertex",
                    size: 2,
                    type_: GL::FLOAT,
                    ..Default::default()
                },
            )],
            None,
        )?;
        let draw_endpoints_buffer = VertexArrayObject::new(
            context,
            &draw_endpoints_program,
            &[(
                &endpoint_vertices,
                VertexBufferLayout {
                    name: "vertex",
                    size: 2,
                    type_: GL::FLOAT,
                    ..Default::default()
                },
            )],
            None,
        )?;

        // Uniforms

        let projection_matrix = new_projection_matrix(screen_width, screen_height);

        let view_matrix = glm::scale(
            &glm::identity(),
            &glm::vec3(settings.view_scale, settings.view_scale, 1.0),
        );

        let projection = Projection {
            projection: projection_matrix.as_slice().try_into().unwrap(),
            view: view_matrix.as_slice().try_into().unwrap(),
        };
        let view_buffer = Buffer::from_f32(
            &context,
            &bytemuck::cast_slice(&[projection]),
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;

        let uniforms = LineUniforms::new(&settings, 0.0);
        let line_uniforms = Buffer::from_f32(
            &context,
            &bytemuck::cast_slice(&[uniforms]),
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;

        // Workaround for iOS
        //
        // Safari on iOS crashes if you use a uniform block buffer together with
        // transform feedback.
        let color_wheel = settings::color_wheel_from_scheme(&settings.color_scheme);
        place_lines_program.set_uniforms(&[
            &Uniform {
                name: "velocityTexture",
                value: UniformValue::Texture2D(0),
            },
            &Uniform {
                name: "uLineFadeOutLength",
                value: UniformValue::Float(settings.line_fade_out_length),
            },
            &Uniform {
                name: "uSpringStiffness",
                value: UniformValue::Float(settings.spring_stiffness),
            },
            &Uniform {
                name: "uSpringVariance",
                value: UniformValue::Float(settings.spring_variance),
            },
            &Uniform {
                name: "uSpringMass",
                value: UniformValue::Float(settings.spring_mass),
            },
            &Uniform {
                name: "uSpringRestLength",
                value: UniformValue::Float(settings.spring_rest_length),
            },
            &Uniform {
                name: "uAdjustAdvection",
                value: UniformValue::Float(settings.adjust_advection),
            },
            &Uniform {
                name: "uColorWheel[0]",
                value: UniformValue::Vec4Array(&color_wheel),
            },
            &Uniform {
                name: "uProjection",
                value: UniformValue::Mat4(&projection_matrix.as_slice()),
            },
        ]);

        draw_lines_program.set_uniform_block("Projection", 0);
        draw_lines_program.set_uniform_block("LineUniforms", 1);
        draw_endpoints_program.set_uniform_block("Projection", 0);
        draw_endpoints_program.set_uniform_block("LineUniforms", 1);

        // Vertex buffers

        let draw_texture_buffer = VertexArrayObject::new(
            &context,
            &draw_texture_program,
            &[(
                &plane_vertices,
                VertexBufferLayout {
                    name: "position",
                    size: 3,
                    type_: GL::FLOAT,
                    ..Default::default()
                },
            )],
            Some(&plane_indices),
        )?;
        draw_texture_program.set_uniform_block("Projection", 0);

        let antialiasing_samples = 2;
        let antialiasing_pass =
            render::MsaaPass::new(context, screen_width, screen_height, antialiasing_samples)?;

        let drawer = Self {
            context: Rc::clone(context),

            screen_width,
            screen_height,
            grid_width,
            grid_height,
            grid_spacing: settings.grid_spacing,
            line_count,

            basepoint_buffer,
            line_state_buffer,
            line_state_feedback_buffer: Buffer::from_f32(
                &context,
                &bytemuck::cast_slice(&line_state),
                GL::ARRAY_BUFFER,
                GL::DYNAMIC_READ,
            )?,
            transform_feedback_buffer,

            place_lines_buffer,
            draw_lines_buffer,
            draw_endpoints_buffer,
            draw_texture_buffer,

            view_buffer,
            line_uniforms,

            place_lines_pass: place_lines_program,
            draw_lines_pass: draw_lines_program,
            draw_endpoints_pass: draw_endpoints_program,
            draw_texture_pass: draw_texture_program,
            antialiasing_pass,
        };

        drawer.update_line_buffers()?;

        Ok(drawer)
    }

    pub fn update(&mut self, settings: &Rc<Settings>) -> () {
        self.context
            .bind_buffer(GL::UNIFORM_BUFFER, Some(&self.line_uniforms.id));

        let uniforms = LineUniforms::new(settings, 0.0);
        self.context.buffer_sub_data_with_i32_and_u8_array(
            GL::UNIFORM_BUFFER,
            0,
            &bytemuck::bytes_of(&uniforms),
        );

        self.context.bind_buffer(GL::UNIFORM_BUFFER, None);

        // Workaround for iOS
        let color_wheel = settings::color_wheel_from_scheme(&settings.color_scheme);
        self.place_lines_pass.set_uniforms(&[
            &Uniform {
                name: "uLineFadeOutLength",
                value: UniformValue::Float(settings.line_fade_out_length),
            },
            &Uniform {
                name: "uSpringStiffness",
                value: UniformValue::Float(settings.spring_stiffness),
            },
            &Uniform {
                name: "uSpringVariance",
                value: UniformValue::Float(settings.spring_variance),
            },
            &Uniform {
                name: "uSpringMass",
                value: UniformValue::Float(settings.spring_mass),
            },
            &Uniform {
                name: "uSpringRestLength",
                value: UniformValue::Float(settings.spring_rest_length),
            },
            &Uniform {
                name: "uAdjustAdvection",
                value: UniformValue::Float(settings.adjust_advection),
            },
            &Uniform {
                name: "uColorWheel[0]",
                value: UniformValue::Vec4Array(&color_wheel),
            },
        ]);
    }

    pub fn resize(&mut self, screen_width: u32, screen_height: u32) -> Result<(), render::Problem> {
        let (grid_width, grid_height) = (screen_width, screen_height);
        let grid_spacing = self.grid_spacing;

        self.screen_width = screen_width;
        self.screen_height = screen_height;
        self.grid_width = screen_width;
        self.grid_height = screen_height;

        self.update_projection(&new_projection_matrix(screen_width, screen_height));
        self.antialiasing_pass.resize(screen_width, screen_height);

        self.line_count = (grid_width / grid_spacing) * (grid_height / grid_spacing);
        let basepoints = new_basepoints(grid_width, grid_height, grid_spacing);
        self.basepoint_buffer = Buffer::from_f32(
            &self.context,
            &basepoints,
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;

        let line_state = new_line_state(grid_width, grid_height, grid_spacing);
        self.line_state_buffer = Buffer::from_f32(
            &self.context,
            &bytemuck::cast_slice(&line_state),
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;

        self.line_state_feedback_buffer = Buffer::from_f32(
            &self.context,
            &bytemuck::cast_slice(&line_state),
            GL::ARRAY_BUFFER,
            GL::DYNAMIC_READ,
        )?;

        self.update_line_buffers()?;

        Ok(())
    }

    fn update_line_buffers(&self) -> Result<(), render::Problem> {
        self.place_lines_buffer.update(
            &self.place_lines_pass,
            &[
                (
                    &self.basepoint_buffer,
                    VertexBufferLayout {
                        name: "basepoint",
                        size: 2,
                        type_: GL::FLOAT,
                        ..Default::default()
                    },
                ),
                (
                    &self.line_state_buffer,
                    VertexBufferLayout {
                        name: "iEndpointVector",
                        size: 2,
                        type_: GL::FLOAT,
                        stride: 10 * 4,
                        offset: 0 * 4,
                        divisor: 0,
                    },
                ),
                (
                    &self.line_state_buffer,
                    VertexBufferLayout {
                        name: "iVelocityVector",
                        size: 2,
                        type_: GL::FLOAT,
                        stride: 10 * 4,
                        offset: 2 * 4,
                        divisor: 0,
                    },
                ),
                (
                    &self.line_state_buffer,
                    VertexBufferLayout {
                        name: "iColor",
                        size: 4,
                        type_: GL::FLOAT,
                        stride: 10 * 4,
                        offset: 4 * 4,
                        divisor: 0,
                    },
                ),
                (
                    &self.line_state_buffer,
                    VertexBufferLayout {
                        name: "iLineWidth",
                        size: 1,
                        type_: GL::FLOAT,
                        stride: 10 * 4,
                        offset: 8 * 4,
                        divisor: 0,
                    },
                ),
                (
                    &self.line_state_buffer,
                    VertexBufferLayout {
                        name: "iOpacity",
                        size: 1,
                        type_: GL::FLOAT,
                        stride: 10 * 4,
                        offset: 9 * 4,
                        divisor: 0,
                    },
                ),
            ],
            None,
        )?;

        let line_state_attribs = [
            (
                &self.basepoint_buffer,
                VertexBufferLayout {
                    name: "basepoint",
                    size: 2,
                    type_: GL::FLOAT,
                    divisor: 1,
                    ..Default::default()
                },
            ),
            (
                &self.line_state_buffer,
                VertexBufferLayout {
                    name: "iEndpointVector",
                    size: 2,
                    type_: GL::FLOAT,
                    stride: 10 * 4,
                    offset: 0 * 4,
                    divisor: 1,
                },
            ),
            (
                &self.line_state_buffer,
                VertexBufferLayout {
                    name: "iVelocityVector",
                    size: 2,
                    type_: GL::FLOAT,
                    stride: 10 * 4,
                    offset: 2 * 4,
                    divisor: 1,
                },
            ),
            (
                &self.line_state_buffer,
                VertexBufferLayout {
                    name: "iColor",
                    size: 4,
                    type_: GL::FLOAT,
                    stride: 10 * 4,
                    offset: 4 * 4,
                    divisor: 1,
                },
            ),
            (
                &self.line_state_buffer,
                VertexBufferLayout {
                    name: "iLineWidth",
                    size: 1,
                    type_: GL::FLOAT,
                    stride: 10 * 4,
                    offset: 8 * 4,
                    divisor: 1,
                },
            ),
            (
                &self.line_state_buffer,
                VertexBufferLayout {
                    name: "iOpacity",
                    size: 1,
                    type_: GL::FLOAT,
                    stride: 10 * 4,
                    offset: 9 * 4,
                    divisor: 1,
                },
            ),
        ];
        self.draw_lines_buffer
            .update(&self.draw_lines_pass, &line_state_attribs, None)?;
        self.draw_endpoints_buffer
            .update(&self.draw_endpoints_pass, &line_state_attribs, None)?;

        Ok(())
    }

    fn update_projection(&self, projection: &glm::TMat4<f32>) {
        let projection: [f32; 16] = projection.as_slice().try_into().unwrap();
        self.context
            .bind_buffer(GL::UNIFORM_BUFFER, Some(&self.view_buffer.id));
        self.context
            .buffer_sub_data_with_i32_and_u8_array_and_src_offset_and_length(
                GL::UNIFORM_BUFFER,
                0 * 4,
                &bytemuck::cast_slice(&projection),
                0,
                16 * 4,
            );
        self.context.bind_buffer(GL::UNIFORM_BUFFER, None);

        // Workaround for iOS
        self.place_lines_pass.set_uniform(&Uniform {
            name: "uProjection",
            value: UniformValue::Mat4(&projection),
        });
    }

    pub fn place_lines(&self, timestep: f32, texture: &Framebuffer) -> () {
        self.context
            .viewport(0, 0, self.screen_width as i32, self.screen_height as i32);
        self.context.disable(GL::BLEND);

        self.place_lines_pass.use_program();

        self.context
            .bind_vertex_array(Some(&self.place_lines_buffer.id));

        self.place_lines_pass.set_uniform(&Uniform {
            name: "deltaT",
            value: UniformValue::Float(timestep),
        });

        self.context.active_texture(GL::TEXTURE0);
        self.context
            .bind_texture(GL::TEXTURE_2D, Some(&texture.texture));

        self.context.bind_transform_feedback(
            GL::TRANSFORM_FEEDBACK,
            Some(&self.transform_feedback_buffer),
        );
        self.context.bind_buffer_base(
            GL::TRANSFORM_FEEDBACK_BUFFER,
            0,
            Some(&self.line_state_feedback_buffer.id),
        );

        self.context.enable(GL::RASTERIZER_DISCARD);
        self.context.begin_transform_feedback(GL::POINTS);

        self.context
            .draw_arrays(GL::POINTS, 0, self.line_count as i32);

        self.context.end_transform_feedback();
        self.context
            .bind_buffer_base(GL::TRANSFORM_FEEDBACK_BUFFER, 0, None);
        self.context
            .bind_transform_feedback(GL::TRANSFORM_FEEDBACK, None);
        self.context.disable(GL::RASTERIZER_DISCARD);

        self.context
            .bind_buffer(GL::COPY_WRITE_BUFFER, Some(&self.line_state_buffer.id));
        self.context.bind_buffer(
            GL::COPY_READ_BUFFER,
            Some(&self.line_state_feedback_buffer.id),
        );
        // Copy new line state
        self.context.copy_buffer_sub_data_with_i32_and_i32_and_i32(
            GL::COPY_READ_BUFFER,
            GL::COPY_WRITE_BUFFER,
            0,
            0,
            (std::mem::size_of::<LineState>() as i32) * (self.line_count as i32),
        );
        self.context.bind_buffer(GL::COPY_READ_BUFFER, None);
        self.context.bind_buffer(GL::COPY_WRITE_BUFFER, None);
    }

    pub fn draw_lines(&self) -> () {
        self.context
            .viewport(0, 0, self.screen_width as i32, self.screen_height as i32);

        self.context.enable(GL::BLEND);
        self.context.blend_func(GL::SRC_ALPHA, GL::ONE);

        self.draw_lines_pass.use_program();
        self.context
            .bind_vertex_array(Some(&self.draw_lines_buffer.id));

        self.context
            .bind_buffer_base(GL::UNIFORM_BUFFER, 0, Some(&self.view_buffer.id));
        self.context
            .bind_buffer_base(GL::UNIFORM_BUFFER, 1, Some(&self.line_uniforms.id));

        self.context
            .draw_arrays_instanced(GL::TRIANGLES, 0, 6, self.line_count as i32);

        self.context.disable(GL::BLEND);
    }

    pub fn draw_endpoints(&self) -> () {
        self.context
            .viewport(0, 0, self.screen_width as i32, self.screen_height as i32);

        self.context.enable(GL::BLEND);
        self.context.blend_func(GL::SRC_ALPHA, GL::ONE);

        self.draw_endpoints_pass.use_program();
        self.context
            .bind_vertex_array(Some(&self.draw_endpoints_buffer.id));

        self.context
            .bind_buffer_base(GL::UNIFORM_BUFFER, 0, Some(&self.view_buffer.id));
        self.context
            .bind_buffer_base(GL::UNIFORM_BUFFER, 1, Some(&self.line_uniforms.id));

        self.context
            .draw_arrays_instanced(GL::TRIANGLE_FAN, 0, 18, self.line_count as i32);

        self.context.disable(GL::BLEND);
    }

    #[allow(dead_code)]
    pub fn draw_texture(&self, texture: &Framebuffer) -> () {
        self.context
            .viewport(0, 0, self.screen_width as i32, self.screen_height as i32);

        self.draw_texture_pass.use_program();

        self.context
            .bind_buffer_base(GL::UNIFORM_BUFFER, 0, Some(&self.view_buffer.id));

        self.context
            .bind_vertex_array(Some(&self.draw_texture_buffer.id));

        self.context.active_texture(GL::TEXTURE0);
        self.context
            .bind_texture(GL::TEXTURE_2D, Some(&texture.texture));

        self.context
            .draw_elements_with_i32(GL::TRIANGLES, 6, GL::UNSIGNED_SHORT, 0);
    }

    pub fn with_antialiasing<T>(&self, draw_call: T) -> ()
    where
        T: Fn() -> (),
    {
        self.antialiasing_pass.draw_to(draw_call);
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

// World space coordinates: zero-centered, width x height
fn new_basepoints(width: u32, height: u32, grid_spacing: u32) -> Vec<f32> {
    let half_width = (width as f32) / 2.0;
    let half_height = (height as f32) / 2.0;

    let rows = height / grid_spacing;
    let cols = width / grid_spacing;
    let mut data = Vec::with_capacity((rows * cols * 2) as usize);

    for v in 0..rows {
        for u in 0..cols {
            let x: f32 = (u * grid_spacing) as f32;
            let y: f32 = (v * grid_spacing) as f32;

            data.push(x - half_width);
            data.push(y - half_height);
        }
    }

    data
}

// World space coordinates: zero-centered, width x height
fn new_line_state(width: u32, height: u32, grid_spacing: u32) -> Vec<LineState> {
    let rows = height / grid_spacing;
    let cols = width / grid_spacing;
    let mut data =
        Vec::with_capacity(std::mem::size_of::<LineState>() / 4 * (rows * cols) as usize);

    for _ in 0..rows {
        for _ in 0..cols {
            data.push(LineState {
                endpoint: [0.0, 0.0],
                velocity: [0.0, 0.0],
                color: [0.0, 0.0, 0.0, 0.0],
                width: 0.1,
                opacity: 0.0,
            });
        }
    }

    data
}

fn new_endpoint(resolution: u32) -> Vec<f32> {
    let mut segments = Vec::with_capacity((resolution * 2 + 1) as usize);

    segments.push(0.0);
    segments.push(0.0);

    for section in 0..=resolution {
        let angle = 2.0 * PI * (section as f32) / (resolution as f32);
        segments.push(angle.cos());
        segments.push(angle.sin());
    }

    segments
}
