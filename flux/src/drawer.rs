use crate::{data, render, settings};
use render::{Buffer, Context, Framebuffer, Indices, Uniform, UniformValue, VertexBufferLayout};
use settings::Settings;

use wasm_bindgen::{JsCast, JsValue};
use web_sys::WebGl2RenderingContext as GL;
use web_sys::{WebGlBuffer, WebGlTransformFeedback, WebGlVertexArrayObject};
extern crate nalgebra_glm as glm;
use bytemuck::{Pod, Zeroable};
use std::rc::Rc;

static LINE_VERT_SHADER: &'static str = include_str!("./shaders/line.vert");
static LINE_FRAG_SHADER: &'static str = include_str!("./shaders/line.frag");
static ENDPOINT_VERT_SHADER: &'static str = include_str!("./shaders/endpoint.vert");
static ENDPOINT_FRAG_SHADER: &'static str = include_str!("./shaders/endpoint.frag");
static TEXTURE_VERT_SHADER: &'static str = include_str!("./shaders/texture.vert");
static TEXTURE_FRAG_SHADER: &'static str = include_str!("./shaders/texture.frag");
static PLACE_LINES_VERT_SHADER: &'static str = include_str!("./shaders/place_lines.vert");
static PLACE_LINES_FRAG_SHADER: &'static str = include_str!("./shaders/place_lines.frag");

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
    line_opacity: f32,
    line_fade_out_length: f32,
    timestep: f32,
    padding: [f32; 2],
    color_wheel: [f32; 24],
}

pub struct Drawer {
    context: Context,
    settings: Rc<Settings>,

    screen_width: u32,
    screen_height: u32,

    pub grid_width: u32,
    pub grid_height: u32,
    pub line_count: u32,

    // A 6-color hue wheel. Each color gets π/3 or 60° of space.
    color_wheel: [f32; 24],

    line_state_buffers: [Buffer; 2],
    transform_feedback_buffer: WebGlTransformFeedback,
    feedback_buffer: Buffer,
    last_line_state_buffer_index: usize,
    place_lines_buffer: [WebGlVertexArrayObject; 2],
    draw_lines_buffer: [WebGlVertexArrayObject; 2],
    draw_endpoints_buffer: [WebGlVertexArrayObject; 2],

    view_buffer: Buffer,
    line_uniforms: Buffer,

    place_lines_pass: render::RenderPipeline,
    draw_lines_pass: render::RenderPipeline,
    draw_endpoints_pass: render::RenderPipeline,
    // draw_texture_pass: render::RenderPipeline,
    antialiasing_pass: render::MsaaPass,

    velocity_textures: render::DoubleFramebuffer,

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
        let line_state_buffers = [
            Buffer::from_f32(&context, &line_state, GL::ARRAY_BUFFER, GL::DYNAMIC_COPY)?,
            Buffer::from_f32(&context, &line_state, GL::ARRAY_BUFFER, GL::DYNAMIC_COPY)?,
        ];

        let circle_vertices = Buffer::from_f32(
            &context,
            &data::new_semicircle(8),
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;

        // let plane_vertices = Buffer::from_f32(
        //     &context,
        //     &data::PLANE_VERTICES.to_vec(),
        //     GL::ARRAY_BUFFER,
        //     GL::STATIC_DRAW,
        // )?;
        // let plane_indices = Buffer::from_u16(
        //     &context,
        //     &data::PLANE_INDICES.to_vec(),
        //     GL::ELEMENT_ARRAY_BUFFER,
        //     GL::STATIC_DRAW,
        // )?;

        // Projection

        let projection_matrix = new_projection_matrix(grid_width, grid_height);

        let view_matrix = glm::scale(
            &glm::identity(),
            &glm::vec3(settings.view_scale, settings.view_scale, 1.0),
        );

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
        // let draw_texture_program =
        //     render::Program::new(&context, (TEXTURE_VERT_SHADER, TEXTURE_FRAG_SHADER))?;

        // Pipelines

        let place_lines_pass = render::RenderPipeline::new(
            &context,
            &[VertexBufferLayout {
                name: "basepoint",
                size: 2,
                type_: GL::FLOAT,
                ..Default::default()
            }],
            &Indices::NoIndices(GL::POINTS),
            &place_lines_program,
        )?;

        let place_lines_buffer = line_state_buffers
            .iter()
            .map(|buffer| {
                let vertices = [
                    (
                        &basepoint_buffer,
                        VertexBufferLayout {
                            name: "basepoint",
                            size: 2,
                            type_: GL::FLOAT,
                            divisor: 1,
                            ..Default::default()
                        },
                    ),
                    (
                        &buffer,
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
                        &buffer,
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
                        &buffer,
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
                        &buffer,
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
                        &buffer,
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
                create_vertex_array(&context, &place_lines_program, &vertices)
            })
            .collect::<Vec<WebGlVertexArrayObject>>();
        let place_lines_buffer = [place_lines_buffer[0].clone(), place_lines_buffer[1].clone()];

        let line_uniforms_block_index =
            context.get_uniform_block_index(&place_lines_program.program, "LineUniforms");
        super::log!(
            "Block index for {}: {}",
            "LineUniforms",
            line_uniforms_block_index
        );
        let view_bufffer_block_index =
            context.get_uniform_block_index(&place_lines_program.program, "Projection");
        super::log!(
            "Block index for {}: {}",
            "Projection",
            view_bufffer_block_index
        );
        super::log!("setting {}", "Projection");
        context.uniform_block_binding(&place_lines_program.program, view_bufffer_block_index, 0);
        super::log!("setting {}", "LineUniforms");
        context.uniform_block_binding(&place_lines_program.program, line_uniforms_block_index, 1);

        let draw_lines_pass = render::RenderPipeline::new(
            &context,
            &[
                VertexBufferLayout {
                    name: "lineVertex",
                    size: 2,
                    type_: GL::FLOAT,
                    ..Default::default()
                },
                VertexBufferLayout {
                    name: "basepoint",
                    size: 2,
                    type_: GL::FLOAT,
                    divisor: 1,
                    ..Default::default()
                },
                VertexBufferLayout {
                    name: "iEndpointVector",
                    size: 2,
                    type_: GL::FLOAT,
                    stride: 10 * 4,
                    offset: 0 * 4,
                    divisor: 1,
                },
                VertexBufferLayout {
                    name: "iVelocityVector",
                    size: 2,
                    type_: GL::FLOAT,
                    stride: 10 * 4,
                    offset: 2 * 4,
                    divisor: 1,
                },
                VertexBufferLayout {
                    name: "iColor",
                    size: 4,
                    type_: GL::FLOAT,
                    stride: 10 * 4,
                    offset: 4 * 4,
                    divisor: 1,
                },
                VertexBufferLayout {
                    name: "iLineWidth",
                    size: 1,
                    type_: GL::FLOAT,
                    stride: 10 * 4,
                    offset: 8 * 4,
                    divisor: 1,
                },
                VertexBufferLayout {
                    name: "iOpacity",
                    size: 1,
                    type_: GL::FLOAT,
                    stride: 10 * 4,
                    offset: 9 * 4,
                    divisor: 1,
                },
            ],
            &Indices::NoIndices(GL::TRIANGLES),
            &draw_lines_program,
        )?;

        let draw_lines_buffer = line_state_buffers
            .iter()
            .map(|buffer| {
                let vertices = [
                    (
                        &line_vertices,
                        VertexBufferLayout {
                            name: "lineVertex",
                            size: 2,
                            type_: GL::FLOAT,
                            ..Default::default()
                        },
                    ),
                    (
                        &basepoint_buffer,
                        VertexBufferLayout {
                            name: "basepoint",
                            size: 2,
                            type_: GL::FLOAT,
                            divisor: 1,
                            ..Default::default()
                        },
                    ),
                    (
                        &buffer,
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
                        &buffer,
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
                        &buffer,
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
                        &buffer,
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
                        &buffer,
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
                create_vertex_array(&context, &draw_lines_pass.program, &vertices)
            })
            .collect::<Vec<WebGlVertexArrayObject>>();
        let draw_lines_buffer = [draw_lines_buffer[0].clone(), draw_lines_buffer[1].clone()];

        let projection = Projection {
            projection: projection_matrix.as_slice().try_into().unwrap(),
            view: view_matrix.as_slice().try_into().unwrap(),
        };
        let view_buffer = Buffer::from_f32_array(
            &context,
            &bytemuck::cast_slice(&[projection]),
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;

        let line_uniforms_block_index =
            context.get_uniform_block_index(&draw_lines_program.program, "LineUniforms");
        super::log!(
            "Block index for {}: {}",
            "LineUniforms",
            line_uniforms_block_index
        );
        let view_bufffer_block_index =
            context.get_uniform_block_index(&draw_lines_program.program, "Projection");
        super::log!(
            "Block index for {}: {}",
            "Projection",
            view_bufffer_block_index
        );
        super::log!("setting {}", "Projection");
        context.uniform_block_binding(&draw_lines_program.program, view_bufffer_block_index, 0);
        super::log!("setting {}", "LineUniforms");
        context.uniform_block_binding(&draw_lines_program.program, line_uniforms_block_index, 1);

        let uniforms = LineUniforms {
            line_width: settings.line_width,
            line_length: settings.line_length,
            line_begin_offset: settings.line_begin_offset,
            line_opacity: settings.line_opacity,
            line_fade_out_length: settings.line_fade_out_length,
            timestep: 0.0,
            padding: [0.0, 0.0],
            color_wheel: settings::color_wheel_from_scheme(&settings.color_scheme),
        };
        let line_uniforms = Buffer::from_f32_array(
            &context,
            &bytemuck::cast_slice(&[uniforms]),
            GL::ARRAY_BUFFER,
            GL::STATIC_DRAW,
        )?;

        let draw_endpoints_pass = render::RenderPipeline::new(
            &context,
            &[
                VertexBufferLayout {
                    name: "vertex",
                    size: 2,
                    type_: GL::FLOAT,
                    ..Default::default()
                },
                VertexBufferLayout {
                    name: "basepoint",
                    size: 2,
                    type_: GL::FLOAT,
                    divisor: 1,
                    ..Default::default()
                },
            ],
            &Indices::NoIndices(GL::TRIANGLE_FAN),
            &draw_endpoints_program,
        )?;
        let draw_endpoints_buffer = line_state_buffers
            .iter()
            .map(|buffer| {
                let vertices = [
                    (
                        &circle_vertices,
                        VertexBufferLayout {
                            name: "vertex",
                            size: 2,
                            type_: GL::FLOAT,
                            ..Default::default()
                        },
                    ),
                    (
                        &basepoint_buffer,
                        VertexBufferLayout {
                            name: "basepoint",
                            size: 2,
                            type_: GL::FLOAT,
                            divisor: 1,
                            ..Default::default()
                        },
                    ),
                    (
                        &buffer,
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
                        &buffer,
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
                        &buffer,
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
                        &buffer,
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
                        &buffer,
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
                create_vertex_array(&context, &draw_endpoints_program, &vertices)
            })
            .collect::<Vec<WebGlVertexArrayObject>>();
        let draw_endpoints_buffer = [
            draw_endpoints_buffer[0].clone(),
            draw_endpoints_buffer[1].clone(),
        ];
        let line_uniforms_block_index =
            context.get_uniform_block_index(&draw_endpoints_program.program, "LineUniforms");
        super::log!(
            "Block index for {}: {}",
            "LineUniforms",
            line_uniforms_block_index
        );
        let view_bufffer_block_index =
            context.get_uniform_block_index(&draw_endpoints_program.program, "Projection");
        super::log!(
            "Block index for {}: {}",
            "Projection",
            view_bufffer_block_index
        );
        super::log!("setting {}", "Projection");
        context.uniform_block_binding(&draw_endpoints_program.program, view_bufffer_block_index, 0);
        super::log!("setting {}", "LineUniforms");
        context.uniform_block_binding(
            &draw_endpoints_program.program,
            line_uniforms_block_index,
            1,
        );

        // let draw_texture_pass = render::RenderPipeline::new(
        //     &context,
        //     &[VertexBufferLayout {
        //         name: "position",
        //         size: 3,
        //         type_: GL::FLOAT,
        //         ..Default::default()
        //     }],
        //     &Indices::IndexBuffer(GL::TRIANGLES),
        //     &draw_texture_program,
        // )?;

        let antialiasing_samples = 0;
        let antialiasing_pass =
            render::MsaaPass::new(context, screen_width, screen_height, antialiasing_samples)?;

        let initial_velocity_data = vec![0.2; (2 * grid_width * grid_height) as usize];
        let velocity_textures = render::DoubleFramebuffer::new(
            &context,
            grid_width,
            grid_height,
            render::TextureOptions {
                mag_filter: GL::LINEAR,
                min_filter: GL::LINEAR,
                format: GL::RG32F,
                ..Default::default()
            },
        )?
        .with_f32_data(&initial_velocity_data)?;

        Ok(Self {
            context: Rc::clone(context),
            settings: Rc::clone(settings),

            screen_width,
            screen_height,
            grid_width,
            grid_height,
            line_count,
            color_wheel: settings::color_wheel_from_scheme(&settings.color_scheme),

            line_state_buffers,
            feedback_buffer: Buffer::from_f32(
                &context,
                &line_state,
                GL::ARRAY_BUFFER,
                GL::DYNAMIC_READ,
            )?,
            transform_feedback_buffer: context.create_transform_feedback().unwrap(),
            velocity_textures,

            last_line_state_buffer_index: 0,
            place_lines_buffer,
            draw_lines_buffer,
            draw_endpoints_buffer,

            view_buffer,
            line_uniforms,

            place_lines_pass,
            draw_lines_pass,
            draw_endpoints_pass,
            // draw_texture_pass,
            antialiasing_pass,

            projection_matrix,
            view_matrix,
        })
    }

    // pub fn place_lines(&self, timestep: f32, texture: &Framebuffer) -> () {
    pub fn place_lines(&mut self, timestep: f32) {
        self.context
            .viewport(0, 0, self.screen_width as i32, self.screen_height as i32);
        self.context.disable(GL::BLEND);

        self.context
            .use_program(Some(&self.place_lines_pass.program.program));
        self.context.bind_vertex_array(Some(
            &self.place_lines_buffer[self.last_line_state_buffer_index],
        ));

        self.context
            .bind_buffer(GL::UNIFORM_BUFFER, Some(&self.line_uniforms.id));
        self.context
            .buffer_sub_data_with_i32_and_u8_array_and_src_offset_and_length(
                GL::UNIFORM_BUFFER,
                5 * 4,
                &timestep.to_ne_bytes(),
                0,
                4,
            );
        self.context.bind_buffer(GL::UNIFORM_BUFFER, None);

        let current_index = self.last_line_state_buffer_index;
        let next_index = 1 - current_index;

        self.context.bind_transform_feedback(
            GL::TRANSFORM_FEEDBACK,
            Some(&self.transform_feedback_buffer),
        );
        self.context.bind_buffer_base(
            GL::TRANSFORM_FEEDBACK_BUFFER,
            0,
            Some(&self.feedback_buffer.id),
        );

        self.context.enable(GL::RASTERIZER_DISCARD);
        self.context.begin_transform_feedback(GL::POINTS);

        self.context
            .bind_buffer_base(GL::UNIFORM_BUFFER, 0, Some(&self.view_buffer.id));
        self.context
            .bind_buffer_base(GL::UNIFORM_BUFFER, 1, Some(&self.line_uniforms.id));

        self.context.active_texture(GL::TEXTURE0);
        self.context.bind_texture(
            GL::TEXTURE_2D,
            Some(&self.velocity_textures.current().texture),
        );

        self.context.uniform1i(
            self.place_lines_pass
                .program
                .get_uniform_location("velocityTexture")
                .as_ref(),
            0 as i32,
        );

        self.context
            .draw_arrays(GL::POINTS, 0, self.line_count as i32);

        self.context.end_transform_feedback();

        self.context.bind_buffer(
            GL::COPY_WRITE_BUFFER,
            Some(&self.line_state_buffers[current_index].id),
        );
        // Copy new buffer
        self.context.copy_buffer_sub_data_with_i32_and_i32_and_i32(
            GL::TRANSFORM_FEEDBACK_BUFFER,
            GL::COPY_WRITE_BUFFER,
            0,
            0,
            10 * 4 * self.line_count as i32,
        );

        self.context
            .bind_buffer_base(GL::TRANSFORM_FEEDBACK_BUFFER, 0, None);
        self.context
            .bind_transform_feedback(GL::TRANSFORM_FEEDBACK, None);
        self.context.disable(GL::RASTERIZER_DISCARD);

        // self.last_line_state_buffer_index = next_index;
    }

    pub fn draw_lines(&self) -> () {
        self.context
            .viewport(0, 0, self.screen_width as i32, self.screen_height as i32);

        self.context.enable(GL::BLEND);
        self.context.blend_func(GL::SRC_ALPHA, GL::ONE);

        self.context
            .use_program(Some(&self.draw_lines_pass.program.program));
        self.context.bind_vertex_array(Some(
            &self.draw_lines_buffer[self.last_line_state_buffer_index],
        ));

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

        self.context
            .use_program(Some(&self.draw_endpoints_pass.program.program));
        self.context.bind_vertex_array(Some(
            &self.draw_endpoints_buffer[self.last_line_state_buffer_index],
        ));

        self.context
            .bind_buffer_base(GL::UNIFORM_BUFFER, 0, Some(&self.view_buffer.id));
        self.context
            .bind_buffer_base(GL::UNIFORM_BUFFER, 1, Some(&self.line_uniforms.id));

        self.context
            .draw_arrays_instanced(GL::TRIANGLE_FAN, 0, 9, self.line_count as i32);

        self.context.disable(GL::BLEND);
    }

    // #[allow(dead_code)]
    // pub fn draw_texture(&self, texture: &Framebuffer) -> () {
    //     self.context
    //         .viewport(0, 0, self.screen_width as i32, self.screen_height as i32);

    //     self.draw_texture_pass
    //         .draw(
    //             &[Uniform {
    //                 name: "inputTexture",
    //                 value: UniformValue::Texture2D(&texture.texture, 0),
    //             }],
    //             1,
    //         )
    //         .unwrap();
    // }

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

fn create_vertex_array(
    context: &Context,
    program: &render::Program,
    vertices: &[(&Buffer, VertexBufferLayout)],
) -> WebGlVertexArrayObject {
    let vao = context.create_vertex_array().unwrap();
    context.bind_vertex_array(Some(&vao));

    for (buffer, vertex) in vertices.iter() {
        bind_attributes(&context, &program, buffer, vertex);
    }

    vao
}

pub fn bind_attributes(
    context: &Context,
    program: &render::Program,
    buffer: &Buffer,
    buffer_layout: &VertexBufferLayout,
) -> Result<(), JsValue> {
    context.bind_buffer(GL::ARRAY_BUFFER, Some(&buffer.id));

    if let Some(location) = program.get_attrib_location(&buffer_layout.name) {
        super::log!("Binding attr {}", buffer_layout.name);
        context.enable_vertex_attrib_array(location);

        match buffer_layout.type_ {
            GL::FLOAT => context.vertex_attrib_pointer_with_i32(
                location,
                buffer_layout.size as i32,
                buffer_layout.type_,
                false,
                buffer_layout.stride as i32,
                buffer_layout.offset as i32,
            ),
            GL::UNSIGNED_SHORT | GL::UNSIGNED_INT | GL::INT => context
                .vertex_attrib_i_pointer_with_i32(
                    location,
                    buffer_layout.size as i32,
                    buffer_layout.type_,
                    buffer_layout.stride as i32,
                    buffer_layout.offset as i32,
                ),
            _ => return Err(JsValue::from_str("Oops")),
        };

        context.vertex_attrib_divisor(location, buffer_layout.divisor);
    }
    Ok(())
}
