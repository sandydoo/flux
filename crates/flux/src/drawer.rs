use crate::{data, render, settings};
use render::{
    Buffer, Context, Framebuffer, Uniform, UniformValue, VertexArrayObject, VertexBufferLayout,
};
use settings::Settings;

extern crate nalgebra_glm as glm;
use bytemuck::{Pod, Zeroable};
use glow::HasContext;
use std::f32::consts::PI;
use std::rc::Rc;

static LINE_VERT_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/line.vert"));
static LINE_FRAG_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/line.frag"));
static ENDPOINT_VERT_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/endpoint.vert"));
static ENDPOINT_FRAG_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/endpoint.frag"));
static TEXTURE_VERT_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/texture.vert"));
static TEXTURE_FRAG_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/texture.frag"));
static PLACE_LINES_VERT_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/place_lines.vert"));
static PLACE_LINES_FRAG_SHADER: &'static str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/place_lines.frag"));

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
    line_opacity: f32,
    endpoint_opacity: f32,
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
    fn new(settings: &Rc<Settings>) -> Self {
        Self {
            line_width: settings.line_width,
            line_length: settings.line_length,
            line_begin_offset: settings.line_begin_offset,
            line_fade_out_length: settings.line_fade_out_length,
        }
    }
}

struct TransformFeedback {
    context: Context,
    pub feedback: glow::TransformFeedback,
    pub buffer: Buffer,
}

impl TransformFeedback {
    pub fn new(context: &Context, data: &[f32]) -> Result<Self, render::Problem> {
        let feedback = unsafe {
            context
                .create_transform_feedback()
                .map_err(|_| render::Problem::OutOfMemory)?
        };
        let buffer =
            render::Buffer::from_f32(context, data, glow::ARRAY_BUFFER, glow::DYNAMIC_DRAW)?;

        unsafe {
            context.bind_transform_feedback(glow::TRANSFORM_FEEDBACK, Some(feedback));
            context.bind_buffer_base(glow::TRANSFORM_FEEDBACK_BUFFER, 0, Some(buffer.id));
            context.bind_transform_feedback(glow::TRANSFORM_FEEDBACK, None);
        }

        Ok(Self {
            context: Rc::clone(context),
            feedback,
            buffer,
        })
    }

    pub fn draw_to<T>(&self, draw_call: T)
    where
        T: Fn() -> (),
    {
        unsafe {
            self.context
                .bind_transform_feedback(glow::TRANSFORM_FEEDBACK, Some(self.feedback));

            self.context.enable(glow::RASTERIZER_DISCARD);
            self.context.begin_transform_feedback(glow::POINTS);

            draw_call();

            self.context.end_transform_feedback();
            self.context
                .bind_transform_feedback(glow::TRANSFORM_FEEDBACK, None);
            self.context.disable(glow::RASTERIZER_DISCARD);
        }
    }
}

struct DoubleTransformFeedback {
    context: Context,
    pub active_buffer: usize,
    buffers: [TransformFeedback; 2],
}

impl DoubleTransformFeedback {
    pub fn new(context: &Context, data: &[f32]) -> Result<Self, render::Problem> {
        let front = TransformFeedback::new(context, data)?;
        let back = TransformFeedback::new(context, data)?;

        Ok(Self {
            context: Rc::clone(context),
            active_buffer: 0,
            buffers: [front, back],
        })
    }

    pub fn current_buffer(&self) -> &TransformFeedback {
        &self.buffers[self.active_buffer]
    }

    pub fn next_buffer(&self) -> &TransformFeedback {
        let next = if self.active_buffer == 0 { 1 } else { 0 };
        // &self.buffers[self.active_buffer | 1]
        &self.buffers[next]
    }

    pub fn swap(&mut self) {
        let next = if self.active_buffer == 0 { 1 } else { 0 };
        self.active_buffer = next;
        // self.active_buffer |= 1;
    }

    pub fn draw_to<T>(&mut self, draw_call: T)
    where
        T: Fn() -> (),
    {
        self.next_buffer().draw_to(draw_call);
        self.swap();
    }
}

pub struct Drawer {
    context: Context,
    settings: Rc<Settings>,

    physical_width: u32,
    physical_height: u32,

    pub grid_width: u32,
    pub grid_height: u32,
    pub line_count: u32,

    basepoint_buffer: Buffer,
    line_state_buffers: DoubleTransformFeedback,

    place_lines_buffers: Vec<VertexArrayObject>,
    draw_lines_buffers: Vec<VertexArrayObject>,
    draw_endpoints_buffers: Vec<VertexArrayObject>,
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
        logical_width: u32,
        logical_height: u32,
        physical_width: u32,
        physical_height: u32,
        settings: &Rc<Settings>,
    ) -> Result<Self, render::Problem> {
        let (grid_width, grid_height) = compute_grid_size(logical_width, logical_height);

        let line_count =
            (grid_width / settings.grid_spacing) * (grid_height / settings.grid_spacing);
        let line_state = new_line_state(grid_width, grid_height, settings.grid_spacing);
        let line_state_buffer = Buffer::from_f32(
            &context,
            &bytemuck::cast_slice(&line_state),
            glow::ARRAY_BUFFER,
            glow::DYNAMIC_DRAW,
        )?;
        let line_vertices = Buffer::from_f32(
            &context,
            &bytemuck::cast_slice(&LINE_VERTICES),
            glow::ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;
        let basepoint_buffer = Buffer::from_f32(
            &context,
            &new_basepoints(grid_width, grid_height, settings.grid_spacing),
            glow::ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;
        let endpoint_vertices = Buffer::from_f32(
            &context,
            &new_endpoint(4),
            glow::ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;
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
                    "vLineOpacity",
                    "vEndpointOpacity",
                ],
                mode: glow::INTERLEAVED_ATTRIBS,
            },
        )?;
        let draw_lines_program =
            render::Program::new(&context, (LINE_VERT_SHADER, LINE_FRAG_SHADER))?;
        let draw_endpoints_program =
            render::Program::new(&context, (ENDPOINT_VERT_SHADER, ENDPOINT_FRAG_SHADER))?;
        let draw_texture_program =
            render::Program::new(&context, (TEXTURE_VERT_SHADER, TEXTURE_FRAG_SHADER))?;

        // Vertex buffers

        // let place_lines_buffer = VertexArrayObject::empty(context)?;
        let draw_lines_buffer = VertexArrayObject::new(
            context,
            &draw_lines_program,
            &[(
                &line_vertices,
                VertexBufferLayout {
                    name: "lineVertex",
                    size: 2,
                    type_: glow::FLOAT,
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
                    type_: glow::FLOAT,
                    ..Default::default()
                },
            )],
            None,
        )?;
        let draw_texture_buffer = VertexArrayObject::new(
            &context,
            &draw_texture_program,
            &[(
                &plane_vertices,
                VertexBufferLayout {
                    name: "position",
                    size: 3,
                    type_: glow::FLOAT,
                    ..Default::default()
                },
            )],
            Some(&plane_indices),
        )?;

        // Uniforms

        let projection_matrix = new_projection_matrix(
            grid_width as f32,
            grid_height as f32,
            physical_width as f32,
            physical_height as f32,
        );

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
            glow::ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;

        let uniforms = LineUniforms::new(&settings);
        let line_uniforms = Buffer::from_f32(
            &context,
            &bytemuck::cast_slice(&[uniforms]),
            glow::ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;

        draw_lines_program.set_uniform_block("Projection", 0);
        draw_lines_program.set_uniform_block("LineUniforms", 1);
        draw_endpoints_program.set_uniform_block("Projection", 0);
        draw_endpoints_program.set_uniform_block("LineUniforms", 1);
        draw_texture_program.set_uniform_block("Projection", 0);

        let antialiasing_samples = 0;
        let antialiasing_pass = render::MsaaPass::new(
            context,
            physical_width,
            physical_height,
            antialiasing_samples,
        )?;

        let mut line_state_buffers =
            DoubleTransformFeedback::new(context, &bytemuck::cast_slice(&line_state))?;

        let mut place_lines_buffers = Vec::with_capacity(2);
        for _ in 0..2 {
            place_lines_buffers.push(
                VertexArrayObject::new(
                    &context,
                    &place_lines_program,
                    &[
                        (
                            &basepoint_buffer,
                            VertexBufferLayout {
                                name: "basepoint",
                                size: 2,
                                type_: glow::FLOAT,
                                ..Default::default()
                            },
                        ),
                        (
                            &line_state_buffers.current_buffer().buffer,
                            VertexBufferLayout {
                                name: "iEndpointVector",
                                size: 2,
                                type_: glow::FLOAT,
                                stride: 11 * 4,
                                offset: 0 * 4,
                                divisor: 0,
                            },
                        ),
                        (
                            &line_state_buffers.current_buffer().buffer,
                            VertexBufferLayout {
                                name: "iVelocityVector",
                                size: 2,
                                type_: glow::FLOAT,
                                stride: 11 * 4,
                                offset: 2 * 4,
                                divisor: 0,
                            },
                        ),
                        (
                            &line_state_buffers.current_buffer().buffer,
                            VertexBufferLayout {
                                name: "iColor",
                                size: 4,
                                type_: glow::FLOAT,
                                stride: 11 * 4,
                                offset: 4 * 4,
                                divisor: 0,
                            },
                        ),
                        (
                            &line_state_buffers.current_buffer().buffer,
                            VertexBufferLayout {
                                name: "iLineWidth",
                                size: 1,
                                type_: glow::FLOAT,
                                stride: 11 * 4,
                                offset: 8 * 4,
                                divisor: 0,
                            },
                        ),
                        (
                            &line_state_buffers.current_buffer().buffer,
                            VertexBufferLayout {
                                name: "iLineOpacity",
                                size: 1,
                                type_: glow::FLOAT,
                                stride: 11 * 4,
                                offset: 9 * 4,
                                divisor: 0,
                            },
                        ),
                        (
                            &line_state_buffers.current_buffer().buffer,
                            VertexBufferLayout {
                                name: "iEndpointOpacity",
                                size: 1,
                                type_: glow::FLOAT,
                                stride: 11 * 4,
                                offset: 10 * 4,
                                divisor: 0,
                            },
                        ),
                    ],
                    None,
                )
                .unwrap(),
            );

            line_state_buffers.swap();
        }

        let mut draw_lines_buffers = Vec::with_capacity(2);
        for _ in 0..2 {
            let line_state_attribs = [
                (
                    &line_vertices,
                    VertexBufferLayout {
                        name: "lineVertex",
                        size: 2,
                        type_: glow::FLOAT,
                        ..Default::default()
                    },
                ),
                (
                    &basepoint_buffer,
                    VertexBufferLayout {
                        name: "basepoint",
                        size: 2,
                        type_: glow::FLOAT,
                        divisor: 1,
                        ..Default::default()
                    },
                ),
                (
                    &line_state_buffers.current_buffer().buffer,
                    VertexBufferLayout {
                        name: "iEndpointVector",
                        size: 2,
                        type_: glow::FLOAT,
                        stride: 11 * 4,
                        offset: 0 * 4,
                        divisor: 1,
                    },
                ),
                (
                    &line_state_buffers.current_buffer().buffer,
                    VertexBufferLayout {
                        name: "iVelocityVector",
                        size: 2,
                        type_: glow::FLOAT,
                        stride: 11 * 4,
                        offset: 2 * 4,
                        divisor: 1,
                    },
                ),
                (
                    &line_state_buffers.current_buffer().buffer,
                    VertexBufferLayout {
                        name: "iColor",
                        size: 4,
                        type_: glow::FLOAT,
                        stride: 11 * 4,
                        offset: 4 * 4,
                        divisor: 1,
                    },
                ),
                (
                    &line_state_buffers.current_buffer().buffer,
                    VertexBufferLayout {
                        name: "iLineWidth",
                        size: 1,
                        type_: glow::FLOAT,
                        stride: 11 * 4,
                        offset: 8 * 4,
                        divisor: 1,
                    },
                ),
                (
                    &line_state_buffers.current_buffer().buffer,
                    VertexBufferLayout {
                        name: "iLineOpacity",
                        size: 1,
                        type_: glow::FLOAT,
                        stride: 11 * 4,
                        offset: 9 * 4,
                        divisor: 1,
                    },
                ),
                (
                    &line_state_buffers.current_buffer().buffer,
                    VertexBufferLayout {
                        name: "iEndpointOpacity",
                        size: 1,
                        type_: glow::FLOAT,
                        stride: 11 * 4,
                        offset: 10 * 4,
                        divisor: 1,
                    },
                ),
            ];
            draw_lines_buffers.push(
                VertexArrayObject::new(context, &draw_lines_program, &line_state_attribs, None)
                    .unwrap(),
            );
            line_state_buffers.swap();

            // self.draw_endpoints_buffer
            //     .update(&self.draw_endpoints_pass, &line_state_attribs, None)?;
        }

        let mut draw_endpoints_buffers = Vec::with_capacity(2);
        for _ in 0..2 {
            let line_state_attribs = [
                (
                    &endpoint_vertices,
                    VertexBufferLayout {
                        name: "vertex",
                        size: 2,
                        type_: glow::FLOAT,
                        ..Default::default()
                    },
                ),
                (
                    &basepoint_buffer,
                    VertexBufferLayout {
                        name: "basepoint",
                        size: 2,
                        type_: glow::FLOAT,
                        divisor: 1,
                        ..Default::default()
                    },
                ),
                (
                    &line_state_buffers.current_buffer().buffer,
                    VertexBufferLayout {
                        name: "iEndpointVector",
                        size: 2,
                        type_: glow::FLOAT,
                        stride: 11 * 4,
                        offset: 0 * 4,
                        divisor: 1,
                    },
                ),
                (
                    &line_state_buffers.current_buffer().buffer,
                    VertexBufferLayout {
                        name: "iVelocityVector",
                        size: 2,
                        type_: glow::FLOAT,
                        stride: 11 * 4,
                        offset: 2 * 4,
                        divisor: 1,
                    },
                ),
                (
                    &line_state_buffers.current_buffer().buffer,
                    VertexBufferLayout {
                        name: "iColor",
                        size: 4,
                        type_: glow::FLOAT,
                        stride: 11 * 4,
                        offset: 4 * 4,
                        divisor: 1,
                    },
                ),
                (
                    &line_state_buffers.current_buffer().buffer,
                    VertexBufferLayout {
                        name: "iLineWidth",
                        size: 1,
                        type_: glow::FLOAT,
                        stride: 11 * 4,
                        offset: 8 * 4,
                        divisor: 1,
                    },
                ),
                (
                    &line_state_buffers.current_buffer().buffer,
                    VertexBufferLayout {
                        name: "iLineOpacity",
                        size: 1,
                        type_: glow::FLOAT,
                        stride: 11 * 4,
                        offset: 9 * 4,
                        divisor: 1,
                    },
                ),
                (
                    &line_state_buffers.current_buffer().buffer,
                    VertexBufferLayout {
                        name: "iEndpointOpacity",
                        size: 1,
                        type_: glow::FLOAT,
                        stride: 11 * 4,
                        offset: 10 * 4,
                        divisor: 1,
                    },
                ),
            ];
            draw_endpoints_buffers.push(
                VertexArrayObject::new(context, &draw_endpoints_program, &line_state_attribs, None)
                    .unwrap(),
            );
            line_state_buffers.swap();
        }

        let drawer = Self {
            context: Rc::clone(context),
            settings: Rc::clone(settings),

            physical_width,
            physical_height,

            grid_width,
            grid_height,
            line_count,

            basepoint_buffer,
            line_state_buffers,

            place_lines_buffers,
            draw_lines_buffers,
            draw_endpoints_buffers,
            draw_texture_buffer,

            view_buffer,
            line_uniforms,

            place_lines_pass: place_lines_program,
            draw_lines_pass: draw_lines_program,
            draw_endpoints_pass: draw_endpoints_program,
            draw_texture_pass: draw_texture_program,
            antialiasing_pass,
        };

        // drawer.update_line_buffers()?;
        drawer.set_place_lines_uniforms(&drawer.place_lines_pass, &settings, &projection_matrix);

        Ok(drawer)
    }

    pub fn update(&mut self, settings: &Rc<Settings>) -> () {
        unsafe {
            self.context
                .bind_buffer(glow::UNIFORM_BUFFER, Some(self.line_uniforms.id));

            let uniforms = LineUniforms::new(settings);
            self.context.buffer_sub_data_u8_slice(
                glow::UNIFORM_BUFFER,
                0,
                &bytemuck::bytes_of(&uniforms),
            );

            self.context.bind_buffer(glow::UNIFORM_BUFFER, None);
        }

        // self.set_place_lines_uniforms(&self.place_lines_pass, &self.settings);
    }

    pub fn resize(
        &mut self,
        logical_width: u32,
        logical_height: u32,
        physical_width: u32,
        physical_height: u32,
    ) -> Result<(), render::Problem> {
        let (grid_width, grid_height) = compute_grid_size(logical_width, logical_height);

        self.physical_width = physical_width;
        self.physical_height = physical_height;
        self.grid_width = grid_width;
        self.grid_height = grid_height;

        self.update_projection(&new_projection_matrix(
            grid_width as f32,
            grid_height as f32,
            physical_width as f32,
            physical_height as f32,
        ));
        self.antialiasing_pass
            .resize(physical_width, physical_height);

        self.line_count =
            (grid_width / self.settings.grid_spacing) * (grid_height / self.settings.grid_spacing);
        let basepoints = new_basepoints(grid_width, grid_height, self.settings.grid_spacing);
        self.basepoint_buffer = Buffer::from_f32(
            &self.context,
            &basepoints,
            glow::ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;

        let line_state = new_line_state(grid_width, grid_height, self.settings.grid_spacing);
        // TODO:update buffers

        // self.update_line_buffers()?;

        Ok(())
    }

    fn set_place_lines_uniforms(
        &self,
        place_lines_program: &render::Program,
        settings: &Settings,
        projection_matrix: &glm::TMat4<f32>,
    ) {
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
                name: "noiseTexture",
                value: UniformValue::Texture2D(1),
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
                name: "uSpringDamping",
                value: UniformValue::Float(settings.spring_damping),
            },
            &Uniform {
                name: "uSpringRestLength",
                value: UniformValue::Float(settings.spring_rest_length),
            },
            &Uniform {
                name: "uMaxLineVelocity",
                value: UniformValue::Float(settings.max_line_velocity),
            },
            &Uniform {
                name: "uAdvectionDirection",
                value: UniformValue::Float(settings.advection_direction),
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
    }

    // fn update_line_buffers(&self) -> Result<(), render::Problem> {
    //     self.place_lines_buffer.update(
    //         &self.place_lines_pass,
    //         &[
    //             (
    //                 &self.basepoint_buffer,
    //                 VertexBufferLayout {
    //                     name: "basepoint",
    //                     size: 2,
    //                     type_: glow::FLOAT,
    //                     ..Default::default()
    //                 },
    //             ),
    //             (
    //                 &self.line_state_buffer,
    //                 VertexBufferLayout {
    //                     name: "iEndpointVector",
    //                     size: 2,
    //                     type_: glow::FLOAT,
    //                     stride: 11 * 4,
    //                     offset: 0 * 4,
    //                     divisor: 0,
    //                 },
    //             ),
    //             (
    //                 &self.line_state_buffer,
    //                 VertexBufferLayout {
    //                     name: "iVelocityVector",
    //                     size: 2,
    //                     type_: glow::FLOAT,
    //                     stride: 11 * 4,
    //                     offset: 2 * 4,
    //                     divisor: 0,
    //                 },
    //             ),
    //             (
    //                 &self.line_state_buffer,
    //                 VertexBufferLayout {
    //                     name: "iColor",
    //                     size: 4,
    //                     type_: glow::FLOAT,
    //                     stride: 11 * 4,
    //                     offset: 4 * 4,
    //                     divisor: 0,
    //                 },
    //             ),
    //             (
    //                 &self.line_state_buffer,
    //                 VertexBufferLayout {
    //                     name: "iLineWidth",
    //                     size: 1,
    //                     type_: glow::FLOAT,
    //                     stride: 11 * 4,
    //                     offset: 8 * 4,
    //                     divisor: 0,
    //                 },
    //             ),
    //             (
    //                 &self.line_state_buffer,
    //                 VertexBufferLayout {
    //                     name: "iLineOpacity",
    //                     size: 1,
    //                     type_: glow::FLOAT,
    //                     stride: 11 * 4,
    //                     offset: 9 * 4,
    //                     divisor: 0,
    //                 },
    //             ),
    //             (
    //                 &self.line_state_buffer,
    //                 VertexBufferLayout {
    //                     name: "iEndpointOpacity",
    //                     size: 1,
    //                     type_: glow::FLOAT,
    //                     stride: 11 * 4,
    //                     offset: 10 * 4,
    //                     divisor: 0,
    //                 },
    //             ),
    //         ],
    //         None,
    //     )?;

    //     let line_state_attribs = [
    //         (
    //             &self.basepoint_buffer,
    //             VertexBufferLayout {
    //                 name: "basepoint",
    //                 size: 2,
    //                 type_: glow::FLOAT,
    //                 divisor: 1,
    //                 ..Default::default()
    //             },
    //         ),
    //         (
    //             &self.line_state_buffer,
    //             VertexBufferLayout {
    //                 name: "iEndpointVector",
    //                 size: 2,
    //                 type_: glow::FLOAT,
    //                 stride: 11 * 4,
    //                 offset: 0 * 4,
    //                 divisor: 1,
    //             },
    //         ),
    //         (
    //             &self.line_state_buffer,
    //             VertexBufferLayout {
    //                 name: "iVelocityVector",
    //                 size: 2,
    //                 type_: glow::FLOAT,
    //                 stride: 11 * 4,
    //                 offset: 2 * 4,
    //                 divisor: 1,
    //             },
    //         ),
    //         (
    //             &self.line_state_buffer,
    //             VertexBufferLayout {
    //                 name: "iColor",
    //                 size: 4,
    //                 type_: glow::FLOAT,
    //                 stride: 11 * 4,
    //                 offset: 4 * 4,
    //                 divisor: 1,
    //             },
    //         ),
    //         (
    //             &self.line_state_buffer,
    //             VertexBufferLayout {
    //                 name: "iLineWidth",
    //                 size: 1,
    //                 type_: glow::FLOAT,
    //                 stride: 11 * 4,
    //                 offset: 8 * 4,
    //                 divisor: 1,
    //             },
    //         ),
    //         (
    //             &self.line_state_buffer,
    //             VertexBufferLayout {
    //                 name: "iLineOpacity",
    //                 size: 1,
    //                 type_: glow::FLOAT,
    //                 stride: 11 * 4,
    //                 offset: 9 * 4,
    //                 divisor: 1,
    //             },
    //         ),
    //         (
    //             &self.line_state_buffer,
    //             VertexBufferLayout {
    //                 name: "iEndpointOpacity",
    //                 size: 1,
    //                 type_: glow::FLOAT,
    //                 stride: 11 * 4,
    //                 offset: 10 * 4,
    //                 divisor: 1,
    //             },
    //         ),
    //     ];
    //     self.draw_lines_buffer
    //         .update(&self.draw_lines_pass, &line_state_attribs, None)?;
    //     self.draw_endpoints_buffer
    //         .update(&self.draw_endpoints_pass, &line_state_attribs, None)?;

    //     Ok(())
    // }

    fn update_projection(&self, projection: &glm::TMat4<f32>) {
        let projection: [f32; 16] = projection.as_slice().try_into().unwrap();

        unsafe {
            self.context
                .bind_buffer(glow::UNIFORM_BUFFER, Some(self.view_buffer.id));
            self.context.buffer_sub_data_u8_slice(
                glow::UNIFORM_BUFFER,
                0,
                &bytemuck::cast_slice(&projection),
            );
            self.context.bind_buffer(glow::UNIFORM_BUFFER, None);
        }

        // Workaround for iOS
        self.place_lines_pass.set_uniform(&Uniform {
            name: "uProjection",
            value: UniformValue::Mat4(&projection),
        });
    }

    pub fn place_lines(
        &mut self,
        velocity_texture: &Framebuffer,
        noise_texture: &Framebuffer,
        delta_blend_progress: f32,
        timestep: f32,
    ) -> () {
        unsafe {
            self.context.viewport(
                0,
                0,
                self.physical_width as i32,
                self.physical_height as i32,
            );
            self.context.disable(glow::BLEND);

            self.place_lines_pass.use_program();

            self.context.bind_vertex_array(Some(
                self.place_lines_buffers[self.line_state_buffers.active_buffer].id,
            ));

            self.place_lines_pass.set_uniform(&Uniform {
                name: "deltaT",
                value: UniformValue::Float(timestep),
            });
            self.place_lines_pass.set_uniform(&Uniform {
                name: "uBlendProgress",
                value: UniformValue::Float(delta_blend_progress),
            });

            self.context.active_texture(glow::TEXTURE0);
            self.context
                .bind_texture(glow::TEXTURE_2D, Some(velocity_texture.texture));

            self.context.active_texture(glow::TEXTURE1);
            self.context
                .bind_texture(glow::TEXTURE_2D, Some(noise_texture.texture));

            self.line_state_buffers.draw_to(|| {
                self.context
                    .draw_arrays(glow::POINTS, 0, self.line_count as i32);
            });
        }
    }

    pub fn draw_lines(&self) -> () {
        unsafe {
            self.context.viewport(
                0,
                0,
                self.physical_width as i32,
                self.physical_height as i32,
            );

            self.context.enable(glow::BLEND);
            self.context.blend_func(glow::SRC_ALPHA, glow::ONE);

            self.draw_lines_pass.use_program();
            self.context.bind_vertex_array(Some(
                self.draw_lines_buffers[self.line_state_buffers.active_buffer].id,
            ));

            self.context
                .bind_buffer_base(glow::UNIFORM_BUFFER, 0, Some(self.view_buffer.id));
            self.context
                .bind_buffer_base(glow::UNIFORM_BUFFER, 1, Some(self.line_uniforms.id));

            self.context
                .draw_arrays_instanced(glow::TRIANGLES, 0, 6, self.line_count as i32);

            self.context.disable(glow::BLEND);
        }
    }

    pub fn draw_endpoints(&self) -> () {
        unsafe {
            self.context.viewport(
                0,
                0,
                self.physical_width as i32,
                self.physical_height as i32,
            );

            self.context.enable(glow::BLEND);
            self.context.blend_func(glow::SRC_ALPHA, glow::ONE);

            self.draw_endpoints_pass.use_program();
            self.context.bind_vertex_array(Some(
                self.draw_endpoints_buffers[self.line_state_buffers.active_buffer].id,
            ));

            self.context
                .bind_buffer_base(glow::UNIFORM_BUFFER, 0, Some(self.view_buffer.id));
            self.context
                .bind_buffer_base(glow::UNIFORM_BUFFER, 1, Some(self.line_uniforms.id));

            self.draw_endpoints_pass.set_uniform(&Uniform {
                name: "uOrientation",
                value: UniformValue::Float(1.0),
            });

            self.context
                .draw_arrays_instanced(glow::TRIANGLE_FAN, 0, 6, self.line_count as i32);

            self.draw_endpoints_pass.set_uniform(&Uniform {
                name: "uOrientation",
                value: UniformValue::Float(-1.0),
            });

            self.context
                .draw_arrays_instanced(glow::TRIANGLE_FAN, 0, 6, self.line_count as i32);

            self.context.disable(glow::BLEND);
        }
    }

    #[allow(dead_code)]
    pub fn draw_texture(&self, texture: &Framebuffer) -> () {
        unsafe {
            self.context.viewport(
                0,
                0,
                self.physical_width as i32,
                self.physical_height as i32,
            );

            self.draw_texture_pass.use_program();

            self.context
                .bind_buffer_base(glow::UNIFORM_BUFFER, 0, Some(self.view_buffer.id));

            self.context
                .bind_vertex_array(Some(self.draw_texture_buffer.id));

            self.context.active_texture(glow::TEXTURE0);
            self.context
                .bind_texture(glow::TEXTURE_2D, Some(texture.texture));

            self.context
                .draw_elements(glow::TRIANGLES, 6, glow::UNSIGNED_SHORT, 0);
        }
    }

    pub fn with_antialiasing<T>(&self, draw_call: T) -> ()
    where
        T: Fn() -> (),
    {
        if self.antialiasing_pass.samples > 0 {
            self.antialiasing_pass.draw_to(draw_call)
        } else {
            draw_call()
        }
    }
}

fn compute_grid_size(logical_width: u32, logical_height: u32) -> (u32, u32) {
    if logical_width > logical_height {
        (u32::max(1280, logical_width), u32::max(800, logical_height))
    } else {
        (u32::max(800, logical_width), u32::max(1280, logical_height))
    }
}

fn new_projection_matrix(
    grid_width: f32,
    grid_height: f32,
    physical_width: f32,
    physical_height: f32,
) -> glm::TMat4<f32> {
    let grid_ratio = grid_width / grid_height;
    let physical_ratio = physical_width / physical_height;

    let (width, height) = if grid_ratio > physical_ratio {
        (grid_height * physical_ratio, grid_height)
    } else {
        (grid_width, grid_width / physical_ratio)
    };

    let half_width = width / 2.0;
    let half_height = height / 2.0;

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
        // Horizontal offset every other row
        let offset_u = if v % 2 == 0 { 0.0 } else { 0.0 };

        for u in 0..cols {
            let x: f32 = (offset_u * grid_spacing as f32) + (u * grid_spacing) as f32;
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
                width: 0.0,
                line_opacity: 1.0,
                endpoint_opacity: 0.0,
            });
        }
    }

    data
}

fn new_endpoint(resolution: u32) -> Vec<f32> {
    let mut segments = Vec::with_capacity(((resolution + 1) * 2) as usize);

    segments.push(0.0);
    segments.push(0.0);

    for section in 0..=resolution {
        let angle = PI * (section as f32) / (resolution as f32);
        segments.push(angle.cos());
        segments.push(angle.sin());
    }

    segments
}
