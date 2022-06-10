use crate::{data, render, settings};
use render::{
    Buffer, Context, Framebuffer, Uniform, UniformBlock, UniformValue, VertexArrayObject,
    VertexBufferLayout,
};
use settings::Settings;

use bytemuck::{Pod, Zeroable};
use crevice::std140::AsStd140;
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
    -0.5, 0.0,
    -0.5, 1.0,
     0.5, 1.0,
    -0.5, 0.0,
     0.5, 1.0,
     0.5, 0.0,
];

#[repr(C)]
#[derive(Clone, Copy)]
struct LineState {
    endpoint: mint::Vector2<f32>,
    velocity: mint::Vector2<f32>,
    color: mint::Vector4<f32>,
    color_velocity: mint::Vector3<f32>,
    width: f32,
}

unsafe impl Zeroable for LineState {}
unsafe impl Pod for LineState {}

#[derive(AsStd140)]
struct Projection {
    fluid_projection_matrix: mint::ColumnMatrix4<f32>,
    projection_matrix: mint::ColumnMatrix4<f32>,
    view_matrix: mint::ColumnMatrix4<f32>,
}

#[derive(AsStd140)]
struct LineUniforms {
    line_width: f32,
    line_length: f32,
    line_begin_offset: f32,
    line_variance: f32,
    line_noise_offset_1: f32,
    line_noise_offset_2: f32,
    line_noise_blend_factor: f32,
    delta_time: f32,
}

impl LineUniforms {
    fn new(settings: &Rc<Settings>) -> Self {
        Self {
            line_width: settings.line_width,
            line_length: settings.line_length,
            line_begin_offset: settings.line_begin_offset,
            line_variance: settings.line_variance,
            line_noise_offset_1: 0.0,
            line_noise_offset_2: 0.0,
            line_noise_blend_factor: 0.0,
            delta_time: 0.0,
        }
    }

    fn update(&mut self, settings: &Rc<Settings>) -> &mut Self {
        self.line_width = settings.line_width;
        self.line_length = settings.line_length;
        self.line_begin_offset = settings.line_begin_offset;
        self.line_variance = settings.line_variance;
        self
    }

    fn set_timestep(&mut self, timestep: f32) -> &mut Self {
        self.delta_time = timestep;
        self
    }

    fn tick(&mut self, elapsed_time: f32) -> &mut Self {
        const BLEND_THRESHOLD: f32 = 2.0;
        const BASE_OFFSET: f32 = 0.0015;

        let perturb = 0.001 * (elapsed_time * std::f32::consts::TAU).sin();
        let offset = BASE_OFFSET + perturb;
        self.line_noise_offset_1 += offset;

        if self.line_noise_offset_1 > BLEND_THRESHOLD {
            self.line_noise_offset_2 += offset;
            self.line_noise_blend_factor += BASE_OFFSET;
        }

        if self.line_noise_blend_factor > 1.0 {
            self.line_noise_offset_1 = self.line_noise_offset_2;
            self.line_noise_offset_2 = 0.0;
            self.line_noise_blend_factor = 0.0;
        }

        self
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

    line_uniforms: UniformBlock<LineUniforms>,
    projection: UniformBlock<Projection>,

    place_lines_pass: render::Program,
    draw_lines_pass: render::Program,
    draw_endpoints_pass: render::Program,
    draw_texture_pass: render::Program,
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

        log::debug!("Basepoint grid size: {}x{}", grid_width, grid_height);

        let (basepoints, line_state, line_count) =
            new_line_grid(grid_width, grid_height, settings.grid_spacing);
        let basepoint_buffer =
            Buffer::from_f32(&context, &basepoints, glow::ARRAY_BUFFER, glow::STATIC_DRAW)?;
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

        let place_lines_pass = render::Program::new_with_transform_feedback(
            &context,
            (PLACE_LINES_VERT_SHADER, PLACE_LINES_FRAG_SHADER),
            &render::TransformFeedbackInfo {
                // The order here must match the order in the buffer!
                names: &[
                    "vEndpointVector",
                    "vVelocityVector",
                    "vColor",
                    "vColorVelocity",
                    "vLineWidth",
                ],
                mode: glow::INTERLEAVED_ATTRIBS,
            },
        )?;
        let draw_lines_pass = render::Program::new(&context, (LINE_VERT_SHADER, LINE_FRAG_SHADER))?;
        let draw_endpoints_pass =
            render::Program::new(&context, (ENDPOINT_VERT_SHADER, ENDPOINT_FRAG_SHADER))?;
        let draw_texture_pass =
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
                            name: "iColorVelocity",
                            size: 3,
                            type_: glow::FLOAT,
                            stride,
                            offset: 8 * 4,
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
                            offset: 11 * 4,
                            divisor,
                        },
                    ),
                ]
            };

            place_lines_buffers.push(VertexArrayObject::new(
                context,
                &place_lines_pass,
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
                &draw_lines_pass,
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
                &draw_endpoints_pass,
                &endpoint_attributes,
                None,
            )?);

            line_state_buffers.swap();
        }

        let draw_texture_buffer = VertexArrayObject::new(
            &context,
            &draw_texture_pass,
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

        let projection = Projection {
            fluid_projection_matrix: new_fluid_projection_matrix(
                grid_width as f32,
                grid_height as f32,
            )
            .into(),
            projection_matrix: new_projection_matrix(
                grid_width as f32,
                grid_height as f32,
                logical_width as f32,
                logical_height as f32,
            )
            .into(),
            view_matrix: nalgebra::Matrix4::new_scaling(settings.view_scale).into(),
        };
        let projection = UniformBlock::new(context, projection, 0, glow::STATIC_DRAW)?;

        let line_uniforms =
            UniformBlock::new(context, LineUniforms::new(&settings), 1, glow::DYNAMIC_DRAW)?;

        place_lines_pass.set_uniforms(&[
            &Uniform {
                name: "velocityTexture",
                value: UniformValue::Texture2D(0),
            },
            &Uniform {
                name: "uColorWheel[0]",
                value: UniformValue::Vec4Array(&settings::color_wheel_from_scheme(
                    &settings.color_scheme,
                )),
            },
        ]);

        place_lines_pass.set_uniform_block("Projection", projection.index);
        place_lines_pass.set_uniform_block("LineUniforms", line_uniforms.index);
        draw_lines_pass.set_uniform_block("Projection", projection.index);
        draw_lines_pass.set_uniform_block("LineUniforms", line_uniforms.index);
        draw_endpoints_pass.set_uniform_block("Projection", projection.index);
        draw_endpoints_pass.set_uniform_block("LineUniforms", line_uniforms.index);
        draw_texture_pass.set_uniform_block("Projection", projection.index);

        Ok(Self {
            context: Rc::clone(context),
            settings: Rc::clone(settings),

            physical_width,
            physical_height,

            grid_width,
            grid_height,
            line_count,

            basepoint_buffer,
            line_state_buffers,
            line_vertices,
            endpoint_vertices,
            plane_vertices,

            place_lines_buffers,
            draw_lines_buffers,
            draw_endpoints_buffers,
            draw_texture_buffer,

            projection,
            line_uniforms,

            place_lines_pass,
            draw_lines_pass,
            draw_endpoints_pass,
            draw_texture_pass,
        })
    }

    pub fn update(&mut self, new_settings: &Rc<Settings>) -> () {
        self.line_uniforms
            .update(|line_uniforms| {
                line_uniforms.update(new_settings);
            })
            .buffer_data();

        // FIX: move into uniform buffer
        self.place_lines_pass.set_uniforms(&[&Uniform {
            name: "uColorWheel[0]",
            value: UniformValue::Vec4Array(&settings::color_wheel_from_scheme(
                &new_settings.color_scheme,
            )),
        }]);
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

        self.projection
            .update(|projection| {
                projection.projection_matrix = new_projection_matrix(
                    grid_width as f32,
                    grid_height as f32,
                    logical_width as f32,
                    logical_height as f32,
                )
                .into();
                projection.fluid_projection_matrix =
                    new_fluid_projection_matrix(grid_width as f32, grid_height as f32).into();
            })
            .buffer_data();

        let (basepoints, line_state, line_count) =
            new_line_grid(grid_width, grid_height, self.settings.grid_spacing);
        self.line_count = line_count;
        self.basepoint_buffer
            .overwrite(bytemuck::cast_slice(&basepoints));
        self.line_state_buffers
            .overwrite_buffer(bytemuck::cast_slice(&line_state))?;

        Ok(())
    }

    pub fn place_lines(
        &mut self,
        velocity_texture: &Framebuffer,
        elapsed_time: f32,
        timestep: f32,
    ) -> () {
        self.line_uniforms
            .update(|line_uniforms| {
                line_uniforms.tick(elapsed_time).set_timestep(timestep);
            })
            .buffer_data();

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
            self.projection.bind();
            self.line_uniforms.bind();

            self.place_lines_pass.set_uniform(&Uniform {
                name: "deltaTime",
                value: UniformValue::Float(timestep),
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

            self.projection.bind();
            self.line_uniforms.bind();

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

            self.projection.bind();
            self.line_uniforms.bind();

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

            self.draw_texture_pass.set_uniforms(&[
                &Uniform {
                    name: "uGridWidth",
                    value: UniformValue::Float(self.grid_width as f32),
                },
                &Uniform {
                    name: "uGridHeight",
                    value: UniformValue::Float(self.grid_height as f32),
                },
            ]);

            self.projection.bind();
            self.draw_texture_buffer.bind();

            self.context.active_texture(glow::TEXTURE0);
            self.context
                .bind_texture(glow::TEXTURE_2D, Some(texture.texture));

            self.context.draw_arrays(glow::TRIANGLES, 0, 6);
        }
    }
}

fn compute_grid_size(logical_width: u32, logical_height: u32) -> (u32, u32) {
    let logical_width = logical_width as f32;
    let logical_height = logical_height as f32;
    let target_width = logical_width.clamp(800.0, 1920.0);
    let target_height = logical_height.clamp(800.0, 1920.0);

    let scale_factor = f32::max(target_width / logical_width, target_height / logical_height);

    // The ratio factor ensures we don’t create grids with ridiculous aspect
    // ratios. Remember, this needs to somehow map onto the square fluid
    // texture.
    let ratio = logical_width / logical_height;
    let ratio_factor = ratio.clamp(0.625, 1.6) / ratio;
    let mut ratio_factor_x = 1.0;
    let mut ratio_factor_y = 1.0;

    if ratio > 1.0 {
        ratio_factor_x = ratio_factor;
    } else {
        ratio_factor_y = 1.0 / ratio_factor;
    }

    (
        (logical_width * scale_factor * ratio_factor_x).round() as u32,
        (logical_height * scale_factor * ratio_factor_y).round() as u32,
    )
}

// Project the basepoints into clipspace to then map 1:1 onto the fluid texture.
fn new_fluid_projection_matrix(grid_width: f32, grid_height: f32) -> nalgebra::Matrix4<f32> {
    let half_grid_width = (grid_width as f32) / 2.0;
    let half_grid_height = (grid_height as f32) / 2.0;
    nalgebra::Matrix4::new_orthographic(
        -half_grid_width,
        half_grid_width,
        -half_grid_height,
        half_grid_height,
        -1.0,
        1.0,
    )
}

// Project the basepoints into clipspace, taking into account the aspect ratios
// of the window and the basepoint grid.
//
// Why does this matter? Let’s say our window is very wide and short. This is
// not a very good size for a grid, because we’d be mapping the entire vertical
// axis of the fluid onto just a few grid rows. Instead, we want to create a
// more rectangular grid, sample the fluid properly, and then clip the grid when
// drawing it.
fn new_projection_matrix(
    grid_width: f32,
    grid_height: f32,
    logical_width: f32,
    logical_height: f32,
) -> nalgebra::Matrix4<f32> {
    let grid_ratio = grid_width / grid_height;
    let logical_ratio = logical_width / logical_height;
    let (width, height) = if grid_ratio > logical_ratio {
        (grid_height * logical_ratio, grid_height)
    } else {
        (grid_width, grid_width / logical_ratio)
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

fn new_line_grid(width: u32, height: u32, grid_spacing: u32) -> (Vec<f32>, Vec<LineState>, u32) {
    let height = height as f32;
    let width = width as f32;
    let grid_spacing = grid_spacing as f32;

    let half_width = width / 2.0;
    let half_height = height / 2.0;

    let aspect = width / height;
    let inverse_aspect = 1.0 / aspect;
    let rows = (height / grid_spacing).ceil() as u32;
    let cols = ((aspect * width) / grid_spacing).ceil() as u32;
    let line_count = rows * cols;

    // World space coordinates: zero-centered, width x height
    let mut basepoints = Vec::with_capacity((line_count * 2) as usize);
    for v in 0..rows {
        for u in 0..cols {
            let x: f32 = (u as f32) * grid_spacing * inverse_aspect;
            let y: f32 = (v as f32) * grid_spacing;

            basepoints.push(x - half_width);
            basepoints.push(y - half_height);
        }
    }

    let mut line_state =
        Vec::with_capacity(std::mem::size_of::<LineState>() / 4 * line_count as usize);
    for _ in 0..rows {
        for _ in 0..cols {
            line_state.push(LineState {
                endpoint: [0.0, 0.0].into(),
                velocity: [0.0, 0.0].into(),
                color: [0.0, 0.0, 0.0, 0.0].into(),
                color_velocity: [0.0, 0.0, 0.0].into(),
                width: 0.0,
            });
        }
    }

    (basepoints, line_state, line_count)
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn is_sane_grid_for_iphone_xr() {
        assert_eq!(compute_grid_size(414, 896), (800, 1280));
    }

    #[test]
    fn is_sane_grid_for_iphone_12_pro() {
        assert_eq!(compute_grid_size(390, 844), (800, 1280));
    }

    #[test]
    fn is_sane_grid_for_macbook_pro_13_with_default_scaling() {
        assert_eq!(compute_grid_size(1280, 800), (1280, 800));
    }

    #[test]
    fn is_sane_grid_for_macbook_pro_15_with_default_scaling() {
        assert_eq!(compute_grid_size(1440, 900), (1440, 900));
    }

    #[test]
    fn is_sane_grid_for_awkward_window_sizes() {
        assert_eq!(compute_grid_size(1280, 172), (1280, 800));
        assert_eq!(compute_grid_size(172, 1280), (800, 1280));
    }

    #[test]
    fn is_sane_grid_for_ultrawide_21_9() {
        assert_eq!(compute_grid_size(3840, 1600), (2560, 1600));
    }
}
