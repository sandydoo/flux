use crate::{data, render, settings};
use render::{
    Buffer, Context, Framebuffer, Uniform, UniformBlock, UniformValue, VertexArrayObject,
    VertexBufferLayout,
};
use settings::Settings;

use bytemuck::{Pod, Zeroable};
use crevice::std140::AsStd140;
use glow::HasContext;
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
struct LineUniforms {
    aspect: f32,
    zoom: f32,
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
    fn new(width: f32, height: f32, settings: &Rc<Settings>) -> Self {
        let line_scale_factor = get_line_scale_factor(width, height);
        Self {
            aspect: width / height,
            zoom: settings.view_scale,
            line_width: settings.view_scale * settings.line_width / line_scale_factor,
            line_length: settings.view_scale * settings.line_length / line_scale_factor,
            line_begin_offset: settings.line_begin_offset,
            line_variance: settings.line_variance,
            line_noise_offset_1: 0.0,
            line_noise_offset_2: 0.0,
            line_noise_blend_factor: 0.0,
            delta_time: 0.0,
        }
    }

    fn update(&mut self, width: f32, height: f32, settings: &Rc<Settings>) -> &mut Self {
        let line_scale_factor = get_line_scale_factor(width, height);
        self.aspect = width / height;
        self.zoom = settings.view_scale;
        self.line_width = settings.view_scale * settings.line_width / line_scale_factor;
        self.line_length = settings.view_scale * settings.line_length / line_scale_factor;
        self.line_begin_offset = settings.line_begin_offset;
        self.line_variance = settings.line_variance;
        self
    }

    fn set_timestep(&mut self, timestep: f32) -> &mut Self {
        self.delta_time = timestep;
        self
    }

    fn tick(&mut self, elapsed_time: f32) -> &mut Self {
        const BLEND_THRESHOLD: f32 = 4.0;
        const BASE_OFFSET: f32 = 0.0015;

        let perturb = 1.0 + 0.2 * (0.010 * elapsed_time * std::f32::consts::TAU).sin();
        let offset = BASE_OFFSET * perturb;
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

fn get_line_scale_factor(width: f32, height: f32) -> f32 {
    (0.5 * width + 0.5 + height).max(1000.0)
}

pub struct Drawer {
    context: Context,
    settings: Rc<Settings>,

    logical_width: u32,
    logical_height: u32,
    physical_width: u32,
    physical_height: u32,

    pub line_count: u32,

    basepoint_buffer: Buffer,
    line_state_buffers: render::DoubleTransformFeedback,
    #[allow(unused)]
    line_vertices: Buffer,
    #[allow(unused)]
    plane_vertices: Buffer,

    place_lines_buffers: Vec<VertexArrayObject>,
    draw_lines_buffers: Vec<VertexArrayObject>,
    draw_endpoints_buffers: Vec<VertexArrayObject>,
    draw_texture_buffer: VertexArrayObject,

    line_uniforms: UniformBlock<LineUniforms>,

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
        let (basepoints, line_state, (cols, rows), line_count) =
            new_line_grid(logical_width, logical_height, settings.grid_spacing);

        log::debug!("Grid size: {}x{}", cols, rows);
        log::debug!("Line count: {}", line_count);

        let basepoint_buffer =
            Buffer::from_f32(&context, &basepoints, glow::ARRAY_BUFFER, glow::STATIC_DRAW)?;
        let line_vertices = Buffer::from_f32(
            &context,
            &bytemuck::cast_slice(&LINE_VERTICES),
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
                &plane_vertices,
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

        let line_uniforms = UniformBlock::new(
            context,
            LineUniforms::new(logical_width as f32, logical_height as f32, &settings),
            0,
            glow::DYNAMIC_DRAW,
        )?;

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

        place_lines_pass.set_uniform_block("LineUniforms", line_uniforms.index);
        draw_lines_pass.set_uniform_block("LineUniforms", line_uniforms.index);
        draw_endpoints_pass.set_uniform_block("LineUniforms", line_uniforms.index);

        Ok(Self {
            context: Rc::clone(context),
            settings: Rc::clone(settings),

            logical_width,
            logical_height,
            physical_width,
            physical_height,

            line_count,
            basepoint_buffer,
            line_state_buffers,
            line_vertices,
            plane_vertices,

            place_lines_buffers,
            draw_lines_buffers,
            draw_endpoints_buffers,
            draw_texture_buffer,

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
                line_uniforms.update(
                    self.logical_width as f32,
                    self.logical_height as f32,
                    new_settings,
                );
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
        self.physical_width = physical_width;
        self.physical_height = physical_height;
        self.logical_width = logical_width;
        self.logical_height = logical_height;

        let (basepoints, line_state, _, line_count) =
            new_line_grid(logical_width, logical_height, self.settings.grid_spacing);
        self.line_count = line_count;
        self.basepoint_buffer
            .overwrite(bytemuck::cast_slice(&basepoints));
        self.line_state_buffers
            .overwrite_buffer(bytemuck::cast_slice(&line_state))?;

        self.line_uniforms
            .update(|line_uniforms| {
                line_uniforms.update(logical_width as f32, logical_height as f32, &self.settings);
            })
            .buffer_data();

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
            self.line_uniforms.bind();

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
            self.line_uniforms.bind();

            self.context
                .draw_arrays_instanced(glow::TRIANGLES, 0, 6, self.line_count as i32);

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
            self.draw_texture_buffer.bind();

            self.context.active_texture(glow::TEXTURE0);
            self.context
                .bind_texture(glow::TEXTURE_2D, Some(texture.texture));

            self.context.draw_arrays(glow::TRIANGLES, 0, 6);
        }
    }
}

fn new_line_grid(
    width: u32,
    height: u32,
    grid_spacing: u32,
) -> (Vec<f32>, Vec<LineState>, (u32, u32), u32) {
    let height = height as f32;
    let width = width as f32;
    let grid_spacing = grid_spacing as f32;

    let cols = (width / grid_spacing).floor() as u32;
    let rows = ((height / width) * cols as f32).floor() as u32;
    let line_count = (rows + 1) * (cols + 1);
    let grid_spacing_x: f32 = 1.0 / (cols as f32);
    let grid_spacing_y: f32 = 1.0 / (rows as f32);

    let mut basepoints = Vec::with_capacity((line_count * 2) as usize);
    let mut line_state =
        Vec::with_capacity(std::mem::size_of::<LineState>() / 4 * line_count as usize);

    for v in 0..=rows {
        for u in 0..=cols {
            basepoints.push(u as f32 * grid_spacing_x);
            basepoints.push(v as f32 * grid_spacing_y);

            line_state.push(LineState {
                endpoint: [0.0, 0.0].into(),
                velocity: [0.0, 0.0].into(),
                color: [0.0, 0.0, 0.0, 0.0].into(),
                color_velocity: [0.0, 0.0, 0.0].into(),
                width: 0.0,
            });
        }
    }

    (basepoints, line_state, (cols + 1, rows + 1), line_count)
}

#[cfg(test)]
mod test {
    use super::*;

    fn create_test_grid(width: u32, height: u32, grid_spacing: u32) -> (u32, u32) {
        let (_, _, grid_size, _) = new_line_grid(width, height, grid_spacing);
        grid_size
    }

    #[test]
    fn is_sane_grid_for_iphone_xr() {
        assert_eq!(create_test_grid(414, 896, 15), (28, 59));
    }

    #[test]
    fn is_sane_grid_for_iphone_12_pro() {
        assert_eq!(create_test_grid(390, 844, 15), (27, 57));
    }

    #[test]
    fn is_sane_grid_for_macbook_pro_13_with_1280_800_scaling() {
        assert_eq!(create_test_grid(1280, 800, 15), (86, 54));
    }

    #[test]
    fn is_sane_grid_for_macbook_pro_15_with_1440_900_scaling() {
        assert_eq!(create_test_grid(1440, 900, 15), (97, 61));
    }

    #[test]
    fn is_sane_grid_for_ultrawide_4k() {
        assert_eq!(create_test_grid(3840, 1600, 15), (257, 107));
    }
}
