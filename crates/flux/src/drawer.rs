use crate::{data, render, settings};
use render::{
    Buffer, Context, Framebuffer, Uniform, UniformValue, VertexArrayObject, VertexBufferLayout,
};
use settings::Settings;

use bytemuck::{Pod, Zeroable};
use crevice::std140::{AsStd140, Std140};
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
#[derive(Clone, Copy)]
struct LineState {
    endpoint: mint::Vector2<f32>,
    velocity: mint::Vector2<f32>,
    color: mint::Vector4<f32>,
    width: f32,
}

unsafe impl Zeroable for LineState {}
unsafe impl Pod for LineState {}

#[derive(AsStd140)]
struct Projection {
    projection_matrix: mint::ColumnMatrix4<f32>,
    view_matrix: mint::ColumnMatrix4<f32>,
}

#[derive(AsStd140)]
struct LineUniforms {
    line_width: f32,
    line_length: f32,
    line_begin_offset: f32,
}

impl LineUniforms {
    fn new(settings: &Rc<Settings>) -> Self {
        Self {
            line_width: settings.line_width,
            line_length: settings.line_length,
            line_begin_offset: settings.line_begin_offset,
        }
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
    elapsed_time: f32,

    // TODO: move to uniform buffer
    line_noise_offset: f32,

    basepoint_buffer: Buffer,
    line_state_buffers: render::DoubleTransformFeedback,
    #[allow(unused)]
    line_vertices: Buffer,
    #[allow(unused)]
    endpoint_vertices: Buffer,
    #[allow(unused)]
    plane_vertices: Buffer,

    place_lines_buffers: Vec<VertexArrayObject>,
    draw_lines_buffers: Vec<VertexArrayObject>,
    draw_endpoints_buffers: Vec<VertexArrayObject>,
    draw_texture_buffer: VertexArrayObject,

    view_buffer: Buffer,
    line_uniforms: Buffer,
    projection: Projection,

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

        let basepoints = &new_basepoints(grid_width, grid_height, settings.grid_spacing);
        let line_count = (basepoints.len() / 2) as u32;
        let basepoint_buffer =
            Buffer::from_f32(&context, &basepoints, glow::ARRAY_BUFFER, glow::STATIC_DRAW)?;
        let line_state = new_line_state(grid_width, grid_height, settings.grid_spacing);
        let line_vertices = Buffer::from_f32(
            &context,
            &bytemuck::cast_slice(&LINE_VERTICES),
            glow::ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;
        let endpoint_vertices = Buffer::from_f32(
            &context,
            &new_endpoint(8),
            glow::ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;
        let plane_vertices = Buffer::from_f32(
            &context,
            &data::PLANE_VERTICES,
            glow::ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;

        // Programs

        let place_lines_program = render::Program::new_with_transform_feedback(
            &context,
            (PLACE_LINES_VERT_SHADER, PLACE_LINES_FRAG_SHADER),
            &render::TransformFeedbackInfo {
                // The order here must match the order in the buffer!
                names: &["vEndpointVector", "vVelocityVector", "vColor", "vLineWidth"],
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

        let mut line_state_buffers =
            render::DoubleTransformFeedback::new(context, bytemuck::cast_slice(&line_state))?;
        let mut place_lines_buffers = Vec::with_capacity(2);
        let mut draw_lines_buffers = Vec::with_capacity(2);
        let mut draw_endpoints_buffers = Vec::with_capacity(2);

        for _ in 0..2 {
            let line_state_buffer = &line_state_buffers.current_buffer().buffer;
            let stride = std::mem::size_of::<LineState>() as u32;
            let common_attributes_with_divisor = |divisor| {
                vec![
                    (
                        &basepoint_buffer,
                        VertexBufferLayout {
                            name: "basepoint",
                            size: 2,
                            type_: glow::FLOAT,
                            divisor,
                            ..Default::default()
                        },
                    ),
                    (
                        line_state_buffer,
                        VertexBufferLayout {
                            name: "iEndpointVector",
                            size: 2,
                            type_: glow::FLOAT,
                            stride,
                            offset: 0 * 4,
                            divisor,
                        },
                    ),
                    (
                        line_state_buffer,
                        VertexBufferLayout {
                            name: "iVelocityVector",
                            size: 2,
                            type_: glow::FLOAT,
                            stride,
                            offset: 2 * 4,
                            divisor,
                        },
                    ),
                    (
                        line_state_buffer,
                        VertexBufferLayout {
                            name: "iColor",
                            size: 4,
                            type_: glow::FLOAT,
                            stride,
                            offset: 4 * 4,
                            divisor,
                        },
                    ),
                    (
                        line_state_buffer,
                        VertexBufferLayout {
                            name: "iLineWidth",
                            size: 1,
                            type_: glow::FLOAT,
                            stride,
                            offset: 8 * 4,
                            divisor,
                        },
                    ),
                ]
            };

            place_lines_buffers.push(VertexArrayObject::new(
                context,
                &place_lines_program,
                &common_attributes_with_divisor(0),
                None,
            )?);

            let mut line_attributes = common_attributes_with_divisor(1);
            line_attributes.push((
                &line_vertices,
                VertexBufferLayout {
                    name: "lineVertex",
                    size: 2,
                    type_: glow::FLOAT,
                    ..Default::default()
                },
            ));
            draw_lines_buffers.push(VertexArrayObject::new(
                context,
                &draw_lines_program,
                &line_attributes,
                None,
            )?);

            let mut endpoint_attributes = common_attributes_with_divisor(1);
            endpoint_attributes.push((
                &endpoint_vertices,
                VertexBufferLayout {
                    name: "vertex",
                    size: 2,
                    type_: glow::FLOAT,
                    ..Default::default()
                },
            ));
            draw_endpoints_buffers.push(VertexArrayObject::new(
                context,
                &draw_endpoints_program,
                &endpoint_attributes,
                None,
            )?);

            line_state_buffers.swap();
        }

        let draw_texture_buffer = VertexArrayObject::new(
            &context,
            &draw_texture_program,
            &[(
                &plane_vertices,
                VertexBufferLayout {
                    name: "position",
                    size: 2,
                    type_: glow::FLOAT,
                    ..Default::default()
                },
            )],
            None,
        )?;

        // Uniforms

        let projection_matrix = new_projection_matrix(
            grid_width as f32,
            grid_height as f32,
            physical_width as f32,
            physical_height as f32,
        );

        let view_matrix = nalgebra::Matrix4::new_scaling(settings.view_scale);

        let projection = Projection {
            projection_matrix: projection_matrix.into(),
            view_matrix: view_matrix.into(),
        };
        let view_buffer = Buffer::from_bytes(
            &context,
            projection.as_std140().as_bytes(),
            glow::ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;

        place_lines_program.set_uniforms(&[
            &Uniform {
                name: "velocityTexture",
                value: UniformValue::Texture2D(0),
            },
            &Uniform {
                name: "uLineVariance",
                value: UniformValue::Float(settings.line_variance),
            },
            &Uniform {
                name: "uColorWheel[0]",
                value: UniformValue::Vec4Array(&settings::color_wheel_from_scheme(
                    &settings.color_scheme,
                )),
            },
        ]);

        let uniforms = LineUniforms::new(&settings);
        let line_uniforms = Buffer::from_bytes(
            &context,
            &uniforms.as_std140().as_bytes(),
            glow::ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;

        place_lines_program.set_uniform_block("Projection", 0);
        place_lines_program.set_uniform_block("LineUniforms", 1);
        draw_lines_program.set_uniform_block("Projection", 0);
        draw_lines_program.set_uniform_block("LineUniforms", 1);
        draw_endpoints_program.set_uniform_block("Projection", 0);
        draw_endpoints_program.set_uniform_block("LineUniforms", 1);
        draw_texture_program.set_uniform_block("Projection", 0);

        let antialiasing_samples = 2;
        let antialiasing_pass = render::MsaaPass::new(
            context,
            physical_width,
            physical_height,
            antialiasing_samples,
        )?;

        Ok(Self {
            context: Rc::clone(context),
            settings: Rc::clone(settings),

            physical_width,
            physical_height,

            grid_width,
            grid_height,
            line_count,
            elapsed_time: 0.0,

            line_noise_offset: 0.0,

            basepoint_buffer,
            line_state_buffers,
            line_vertices,
            endpoint_vertices,
            plane_vertices,

            place_lines_buffers,
            draw_lines_buffers,
            draw_endpoints_buffers,
            draw_texture_buffer,

            view_buffer,
            line_uniforms,
            projection,

            place_lines_pass: place_lines_program,
            draw_lines_pass: draw_lines_program,
            draw_endpoints_pass: draw_endpoints_program,
            draw_texture_pass: draw_texture_program,
            antialiasing_pass,
        })
    }

    pub fn update(&mut self, new_settings: &Rc<Settings>) -> () {
        let uniforms = LineUniforms::new(new_settings);
        self.line_uniforms.update(uniforms.as_std140().as_bytes());

        // FIX: move into uniform buffer
        self.place_lines_pass.set_uniforms(&[
            &Uniform {
                name: "uLineVariance",
                value: UniformValue::Float(new_settings.line_variance),
            },
            &Uniform {
                name: "uColorWheel[0]",
                value: UniformValue::Vec4Array(&settings::color_wheel_from_scheme(
                    &new_settings.color_scheme,
                )),
            },
        ]);
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

        self.projection.projection_matrix = new_projection_matrix(
            grid_width as f32,
            grid_height as f32,
            physical_width as f32,
            physical_height as f32,
        )
        .into();
        self.view_buffer
            .update(self.projection.as_std140().as_bytes());

        self.antialiasing_pass
            .resize(physical_width, physical_height);

        let basepoints = new_basepoints(grid_width, grid_height, self.settings.grid_spacing);
        self.basepoint_buffer
            .overwrite(bytemuck::cast_slice(&basepoints));

        self.line_count = (basepoints.len() / 2) as u32;
        let line_state = new_line_state(grid_width, grid_height, self.settings.grid_spacing);
        self.line_state_buffers
            .overwrite_buffer(bytemuck::cast_slice(&line_state))?;

        Ok(())
    }

    pub fn place_lines(&mut self, velocity_texture: &Framebuffer, timestep: f32) -> () {
        self.elapsed_time += timestep;

        let perturb = 0.001 * (self.elapsed_time.to_radians() - std::f32::consts::PI).sin();
        self.line_noise_offset += 0.0015 + perturb;

        unsafe {
            self.context.viewport(
                0,
                0,
                self.physical_width as i32,
                self.physical_height as i32,
            );
            self.context.disable(glow::BLEND);

            self.place_lines_pass.use_program();

            self.place_lines_buffers[self.line_state_buffers.active_buffer].bind();
            self.context
                .bind_buffer_base(glow::UNIFORM_BUFFER, 0, Some(self.view_buffer.id));
            self.context
                .bind_buffer_base(glow::UNIFORM_BUFFER, 1, Some(self.line_uniforms.id));

            self.place_lines_pass.set_uniform(&Uniform {
                name: "deltaTime",
                value: UniformValue::Float(timestep),
            });
            self.place_lines_pass.set_uniform(&Uniform {
                name: "lineNoiseOffset1",
                value: UniformValue::Float(self.line_noise_offset),
            });

            self.context.active_texture(glow::TEXTURE0);
            self.context
                .bind_texture(glow::TEXTURE_2D, Some(velocity_texture.texture));

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
            self.draw_lines_buffers[self.line_state_buffers.active_buffer].bind();

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
            self.draw_endpoints_buffers[self.line_state_buffers.active_buffer].bind();

            self.context
                .bind_buffer_base(glow::UNIFORM_BUFFER, 0, Some(self.view_buffer.id));
            self.context
                .bind_buffer_base(glow::UNIFORM_BUFFER, 1, Some(self.line_uniforms.id));

            self.draw_endpoints_pass.set_uniform(&Uniform {
                name: "uOrientation",
                value: UniformValue::Float(1.0),
            });

            self.context
                .draw_arrays_instanced(glow::TRIANGLE_FAN, 0, 10, self.line_count as i32);

            self.draw_endpoints_pass.set_uniform(&Uniform {
                name: "uOrientation",
                value: UniformValue::Float(-1.0),
            });

            self.context
                .draw_arrays_instanced(glow::TRIANGLE_FAN, 0, 10, self.line_count as i32);

            self.context.disable(glow::BLEND);
        }
    }

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
            self.draw_texture_buffer.bind();

            self.context.active_texture(glow::TEXTURE0);
            self.context
                .bind_texture(glow::TEXTURE_2D, Some(texture.texture));

            self.context.draw_arrays(glow::TRIANGLES, 0, 6);
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
) -> nalgebra::Matrix4<f32> {
    let grid_ratio = grid_width / grid_height;
    let physical_ratio = physical_width / physical_height;

    let (width, height) = if grid_ratio > physical_ratio {
        (grid_height * physical_ratio, grid_height)
    } else {
        (grid_width, grid_width / physical_ratio)
    };

    let half_width = width / 2.0;
    let half_height = height / 2.0;

    nalgebra::Matrix4::new_orthographic(
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
                endpoint: [0.0, 0.0].into(),
                velocity: [0.0, 0.0].into(),
                color: [0.0, 0.0, 0.0, 0.0].into(),
                width: 0.0,
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
