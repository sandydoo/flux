use fnv::FnvHasher;
use std::cell::{Ref, RefCell};
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::rc::Rc;
use thiserror::Error;

use js_sys::WebAssembly;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    WebGl2RenderingContext as GL, WebGlBuffer, WebGlFramebuffer, WebGlProgram, WebGlShader,
    WebGlTexture, WebGlTransformFeedback, WebGlUniformLocation, WebGlVertexArrayObject,
};

pub type Context = Rc<GL>;
type GlDataType = u32;
type Result<T> = std::result::Result<T, Problem>;

#[derive(Error, Debug)]
pub enum Problem {
    #[error("Cannot create buffer")]
    CannotCreateBuffer,

    #[error("Cannot create texture")]
    CannotCreateTexture,

    #[error("Cannot create framebuffer")]
    CannotCreateFramebuffer,

    #[error("Cannot create renderbuffer")]
    CannotCreateRenderbuffer,

    #[error("{}", match .0 {
        Some(n) => format!("Cannot create shader: {}", n),
        None => format!("Cannot create shader"),
    })]
    CannotCreateShader(Option<String>),

    #[error("Cannot create program")]
    CannotCreateProgram,

    #[error("Cannot link program: {0}")]
    CannotLinkProgram(String),

    #[error("Cannot write to texture")]
    CannotWriteToTexture,

    #[error("Unexpected data size. Expected: {expected:?}. Actual: {actual:?} ")]
    WrongDataSize { expected: usize, actual: usize },

    #[error("Cannot write to texture")]
    UnsupportedTextureFormat,

    #[error("Can’t find the attribute {0}")]
    CannotFindAttributeBinding(String),

    #[error("The vertex buffers have different numbers of vertices")]
    VerticesCountMismatch,

    #[error("Vertex attribute type is not supported")]
    CannotBindUnsupportedVertexType,
}

#[derive(Clone, Debug)]
pub struct Buffer {
    context: Context,
    pub id: WebGlBuffer,
    pub size: usize,
    pub type_: u32,
}

#[allow(dead_code)]
impl Buffer {
    pub fn from_f32(
        context: &Context,
        data: &Vec<f32>,
        buffer_type: u32,
        usage: u32,
    ) -> Result<Self> {
        let memory_buffer = wasm_bindgen::memory()
            .dyn_into::<WebAssembly::Memory>()
            .unwrap() // fix
            .buffer();
        let arr_location = data.as_ptr() as u32 / 4;
        let data_array = js_sys::Float32Array::new(&memory_buffer)
            .subarray(arr_location, arr_location + data.len() as u32);

        let buffer = context.create_buffer().ok_or(Problem::CannotCreateBuffer)?;

        context.bind_buffer(buffer_type, Some(&buffer));
        context.buffer_data_with_array_buffer_view(buffer_type, &data_array, usage);
        context.bind_buffer(buffer_type, None);

        Ok(Self {
            context: context.clone(),
            id: buffer,
            size: data.len(),
            type_: buffer_type,
        })
    }

    pub fn from_u16(
        context: &Context,
        data: &Vec<u16>,
        buffer_type: u32,
        usage: u32,
    ) -> Result<Self> {
        let memory_buffer = wasm_bindgen::memory()
            .dyn_into::<WebAssembly::Memory>()
            .unwrap() // fix
            .buffer();
        let data_location = data.as_ptr() as u32 / 2;
        let data_array = js_sys::Uint16Array::new(&memory_buffer)
            .subarray(data_location, data_location + data.len() as u32);

        let buffer = context.create_buffer().ok_or(Problem::CannotCreateBuffer)?;

        context.bind_buffer(buffer_type, Some(&buffer));
        context.buffer_data_with_array_buffer_view(buffer_type, &data_array, usage);
        context.bind_buffer(buffer_type, None);

        Ok(Self {
            context: context.clone(),
            id: buffer,
            size: data.len(),
            type_: buffer_type,
        })
    }

    pub fn from_u32(
        context: &Context,
        data: &Vec<u32>,
        buffer_type: u32,
        usage: u32,
    ) -> Result<Self> {
        let memory_buffer = wasm_bindgen::memory()
            .dyn_into::<WebAssembly::Memory>()
            .unwrap() // fix
            .buffer();
        let data_location = data.as_ptr() as u32 / 4;
        let data_array = js_sys::Uint16Array::new(&memory_buffer)
            .subarray(data_location, data_location + data.len() as u32);

        let buffer = context.create_buffer().ok_or(Problem::CannotCreateBuffer)?;

        context.bind_buffer(buffer_type, Some(&buffer));
        context.buffer_data_with_array_buffer_view(buffer_type, &data_array, usage);
        context.bind_buffer(buffer_type, None);

        Ok(Self {
            context: context.clone(),
            id: buffer,
            size: data.len(),
            type_: buffer_type,
        })
    }
}

#[derive(Clone, Copy)]
pub struct TextureOptions {
    pub mag_filter: GlDataType,
    pub min_filter: GlDataType,
    pub wrap_s: GlDataType,
    pub wrap_t: GlDataType,
    pub format: GlDataType,
}

impl Default for TextureOptions {
    fn default() -> Self {
        TextureOptions {
            mag_filter: GL::NEAREST,
            min_filter: GL::NEAREST,
            wrap_s: GL::CLAMP_TO_EDGE,
            wrap_t: GL::CLAMP_TO_EDGE,
            format: GL::RGBA32F,
        }
    }
}

#[derive(Clone)]
pub struct Framebuffer {
    context: Context,
    pub id: WebGlFramebuffer,
    pub width: u32,
    pub height: u32,
    pub texture: WebGlTexture,
    pub options: TextureOptions,
}

impl Framebuffer {
    pub fn new(
        context: &Context,
        width: u32,
        height: u32,
        options: TextureOptions,
    ) -> Result<Self> {
        Self::with_params(&context, width, height, options)
    }

    fn with_params(
        context: &Context,
        width: u32,
        height: u32,
        options: TextureOptions,
    ) -> Result<Self> {
        let texture = context
            .create_texture()
            .ok_or(Problem::CannotCreateTexture)?;

        context.bind_texture(GL::TEXTURE_2D, Some(&texture));
        context.tex_parameteri(
            GL::TEXTURE_2D,
            GL::TEXTURE_MAG_FILTER,
            options.mag_filter as i32,
        );
        context.tex_parameteri(
            GL::TEXTURE_2D,
            GL::TEXTURE_MIN_FILTER,
            options.min_filter as i32,
        );
        context.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_S, options.wrap_s as i32);
        context.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_T, options.wrap_t as i32);
        context.bind_texture(GL::TEXTURE_2D, None);

        let framebuffer = context
            .create_framebuffer()
            .ok_or(Problem::CannotCreateFramebuffer)?;

        Ok(Self {
            context: context.clone(),
            id: framebuffer,
            width,
            height,
            texture,
            options,
        })
    }

    pub fn with_f32_data(self, data: &Vec<f32>) -> Result<Self> {
        let TextureFormat {
            internal_format,
            format,
            type_,
            size,
        } = detect_texture_format(self.options.format)?;

        let expected_size = size * ((self.width * self.height) as usize);
        if data.len() != expected_size {
            return Err(Problem::WrongDataSize {
                expected: expected_size,
                actual: data.len(),
            });
        }

        self.context
            .bind_texture(GL::TEXTURE_2D, Some(&self.texture));
        unsafe {
            let array = js_sys::Float32Array::view(data);
            self.context.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_array_buffer_view(
                GL::TEXTURE_2D,
                0,
                internal_format as i32,
                self.width as i32,
                self.height as i32,
                0,
                format,
                type_,
                Some(&array),
            ).or(Err(Problem::CannotWriteToTexture))?;
        }
        self.context.bind_texture(GL::TEXTURE_2D, None);

        self.context
            .bind_framebuffer(GL::FRAMEBUFFER, Some(&self.id));
        self.context.framebuffer_texture_2d(
            GL::FRAMEBUFFER,
            GL::COLOR_ATTACHMENT0,
            GL::TEXTURE_2D,
            Some(&self.texture),
            0,
        );
        self.context.bind_framebuffer(GL::FRAMEBUFFER, None);

        Ok(self)
    }

    pub fn zero_out(&self) -> Result<()> {
        self.clear_color_with([0.0, 0.0, 0.0, 0.0])
    }

    pub fn clear_color_with(&self, color: [f32; 4]) -> Result<()> {
        self.context
            .bind_framebuffer(GL::FRAMEBUFFER, Some(&self.id));

        self.context
            .viewport(0, 0, self.width as i32, self.height as i32);
        self.context
            .clear_color(color[0], color[1], color[2], color[3]);
        self.context
            .clear(GL::COLOR_BUFFER_BIT | GL::DEPTH_BUFFER_BIT);

        self.context.bind_framebuffer(GL::FRAMEBUFFER, None);

        Ok(())
    }
}

pub struct DoubleFramebuffer {
    pub width: u32,
    pub height: u32,
    front: RefCell<Framebuffer>,
    back: RefCell<Framebuffer>,
}

impl DoubleFramebuffer {
    pub fn new(
        context: &Context,
        width: u32,
        height: u32,
        options: TextureOptions,
    ) -> Result<Self> {
        let front = Framebuffer::new(&context, width, height, options)?;
        let back = Framebuffer::new(&context, width, height, options)?;
        Ok(Self {
            width,
            height,
            front: RefCell::new(front),
            back: RefCell::new(back),
        })
    }

    pub fn with_f32_data(self, data: &Vec<f32>) -> Result<Self> {
        // TODO: are these clones okay? The problem is that the builder pattern
        // doesn’t work well with RefCell in the DoubleBuffer. Another option is
        // to build with references and call a `finalize` method at the end.
        self.front
            .replace_with(|buffer| buffer.clone().with_f32_data(&data).unwrap());
        // TODO: should we copy the data to the second buffer/texture, or just init with the right size?
        self.back
            .replace_with(|buffer| buffer.clone().with_f32_data(&data).unwrap());

        Ok(self)
    }

    pub fn zero_out(&self) -> Result<()> {
        self.current().zero_out()?;
        self.next().zero_out()?;
        Ok(())
    }

    // pub fn clear_color_with(&self, color: [f32; 4]) -> Result<()> {
    //     self.current().clear_color_with(color)?;
    //     self.next().clear_color_with(color)?;
    //     Ok(())
    // }

    pub fn current(&self) -> Ref<Framebuffer> {
        self.front.borrow()
    }

    pub fn next(&self) -> Ref<Framebuffer> {
        self.back.borrow()
    }

    pub fn swap(&self) -> () {
        self.front.swap(&self.back);
    }
}

#[derive(Clone)]
pub struct Program {
    context: Context,
    program: WebGlProgram,
    attributes: HashMap<String, AttributeInfo, BuildHasherDefault<FnvHasher>>,
    uniforms: HashMap<String, UniformInfo, BuildHasherDefault<FnvHasher>>,
}

impl Program {
    pub fn new(context: &Context, shaders: (&str, &str)) -> Result<Self> {
        Self::new_impl(&context, shaders, None)
    }

    pub fn new_with_transform_feedback(
        context: &Context,
        shaders: (&str, &str),
        transform_feedback: TransformFeedback,
    ) -> Result<Self> {
        Self::new_impl(&context, shaders, Some(transform_feedback))
    }

    pub fn new_impl(
        context: &Context,
        shaders: (&str, &str),
        transform_feedback: Option<TransformFeedback>,
    ) -> Result<Self> {
        let vertex_shader = compile_shader(&context, GL::VERTEX_SHADER, shaders.0)?;
        let fragment_shader = compile_shader(&context, GL::FRAGMENT_SHADER, shaders.1)?;

        let program = context
            .create_program()
            .ok_or(Problem::CannotCreateProgram)?;
        context.attach_shader(&program, &vertex_shader);
        context.attach_shader(&program, &fragment_shader);

        if let Some(TransformFeedback { ref names, mode }) = transform_feedback {
            context.transform_feedback_varyings(
                &program,
                &JsValue::from_serde(&names).unwrap(),
                mode,
            );
        }

        context.link_program(&program);

        if !context
            .get_program_parameter(&program, GL::LINK_STATUS)
            .as_bool()
            .unwrap_or(false)
        {
            return Err(Problem::CannotLinkProgram(
                context.get_program_info_log(&program).unwrap().to_string(),
            ));
        }

        // Delete the shaders to free up memory
        context.detach_shader(&program, &vertex_shader);
        context.detach_shader(&program, &fragment_shader);
        context.delete_shader(Some(&vertex_shader));
        context.delete_shader(Some(&fragment_shader));

        // Get attribute locations
        let mut attributes = HashMap::with_hasher(Default::default());
        let attribute_count = context
            .get_program_parameter(&program, GL::ACTIVE_ATTRIBUTES)
            .as_f64()
            .unwrap() as u32;
        for num in 0..attribute_count {
            let info = context.get_active_attrib(&program, num).unwrap();
            let location = context.get_attrib_location(&program, &info.name());
            attributes.insert(
                info.name(),
                AttributeInfo {
                    type_: info.type_(),
                    size: info.size() as u32,
                    location: location as u32,
                },
            );
        }

        // Get uniform locations
        let mut uniforms = HashMap::with_hasher(Default::default());
        let uniform_count = context
            .get_program_parameter(&program, GL::ACTIVE_UNIFORMS)
            .as_f64()
            .unwrap() as u32;
        for num in 0..uniform_count {
            let info = context.get_active_uniform(&program, num).unwrap();
            let location = context
                .get_uniform_location(&program, &info.name())
                .unwrap();
            uniforms.insert(
                info.name(),
                UniformInfo {
                    type_: info.type_(),
                    size: info.size(),
                    location,
                },
            );
        }

        Ok(Program {
            context: context.clone(),
            program,
            attributes,
            uniforms,
        })
    }

    // Move to uniform impl instead? Or not
    pub fn set_uniform(&self, uniform: &Uniform) {
        let context = &self.context;
        context.use_program(Some(&self.program));

        match uniform.value {
            UniformValue::UnsignedInt(value) => {
                context.uniform1ui(self.get_uniform_location(&uniform.name).as_ref(), value)
            }

            UniformValue::SignedInt(value) => {
                context.uniform1i(self.get_uniform_location(&uniform.name).as_ref(), value)
            }

            UniformValue::Float(value) => {
                context.uniform1f(self.get_uniform_location(&uniform.name).as_ref(), value)
            }

            UniformValue::Vec2(value) => context.uniform2fv_with_f32_array(
                self.get_uniform_location(&uniform.name).as_ref(),
                &value,
            ),

            UniformValue::Vec3(value) => context.uniform3fv_with_f32_array(
                self.get_uniform_location(&uniform.name).as_ref(),
                &value,
            ),

            UniformValue::Vec3Array(ref value) => context.uniform3fv_with_f32_array(
                self.get_uniform_location(&uniform.name).as_ref(),
                &value,
            ),

            UniformValue::Mat4(value) => context.uniform_matrix4fv_with_f32_array(
                self.get_uniform_location(&uniform.name).as_ref(),
                false,
                &value,
            ),

            UniformValue::Texture2D(ref texture, id) => {
                context.active_texture(GL::TEXTURE0 + id);
                context.bind_texture(GL::TEXTURE_2D, Some(&texture));

                context.uniform1i(self.get_uniform_location(&uniform.name).as_ref(), id as i32);
            }
        }
    }

    pub fn get_attrib_location(&self, name: &str) -> Option<u32> {
        self.attributes
            .get(name)
            .and_then(|info| Some(info.location))
    }

    pub fn get_uniform_location(&self, name: &str) -> Option<WebGlUniformLocation> {
        self.uniforms
            .get(name)
            .and_then(|info| Some(info.location.clone()))
    }
}

#[derive(Clone)]
struct AttributeInfo {
    type_: u32,
    size: u32,
    location: u32,
}

#[derive(Clone)]
struct UniformInfo {
    type_: u32,
    size: i32,
    location: WebGlUniformLocation,
}

#[derive(Default)]
pub struct Attribute {
    pub location: Option<u32>,
    pub data_type: GlDataType,
    pub divisor: u32,
}

#[allow(dead_code)]
#[derive(Clone)]
pub enum UniformValue<'a> {
    SignedInt(i32),
    UnsignedInt(u32),
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    // TODO: use nalgebra types here
    Vec3Array(&'a [f32]),
    Mat4(&'a [f32]),
    Texture2D(&'a WebGlTexture, u32),
}

pub struct Uniform<'a> {
    pub name: &'static str,
    pub value: UniformValue<'a>,
}

pub struct TransformFeedback {
    pub names: Vec<String>,
    pub mode: u32,
}

pub fn compile_shader(context: &GL, shader_type: u32, source: &str) -> Result<WebGlShader> {
    let shader = context
        .create_shader(shader_type)
        .ok_or(Problem::CannotCreateShader(None))?;
    context.shader_source(&shader, source);
    context.compile_shader(&shader);

    if context
        .get_shader_parameter(&shader, GL::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        Err(Problem::CannotCreateShader(Some(
            context.get_shader_info_log(&shader).unwrap(),
        )))
    }
}

pub struct VertexBuffer {
    pub buffer: Buffer,
    pub binding: BindingInfo,
}

#[derive(Default)]
pub struct BindingInfo {
    pub name: String,
    pub size: u32,
    pub type_: u32,
    pub divisor: u32,
    pub stride: u32,
    pub offset: u32,
}

pub enum Indices {
    IndexBuffer { buffer: Buffer, primitive: u32 },
    NoIndices(u32),
}

pub struct TransformFeedbackBuffer {
    id: WebGlTransformFeedback,
    front: RefCell<Buffer>,
    back: RefCell<Buffer>,
}

impl TransformFeedbackBuffer {
    pub fn new_with_f32(context: &Context, data: &Vec<f32>, usage: u32) -> Result<Self> {
        let transform_feedback_buffer = context.create_transform_feedback().unwrap();

        let front = Buffer::from_f32(&context, &data, GL::ARRAY_BUFFER, usage)?;
        let back = Buffer::from_f32(&context, &data, GL::ARRAY_BUFFER, usage)?;

        Ok(Self {
            id: transform_feedback_buffer,
            front: RefCell::new(front),
            back: RefCell::new(back),
        })
    }

    pub fn current(&self) -> Ref<Buffer> {
        self.front.borrow()
    }

    pub fn next(&self) -> Ref<Buffer> {
        self.back.borrow()
    }

    pub fn swap(&self) {
        self.front.swap(&self.back)
    }
}

pub struct RenderPass {
    context: Context,
    vertex_buffers: Vec<VertexBuffer>,
    indices: Indices,
    program: Program,
    vao: WebGlVertexArrayObject,
}

impl RenderPass {
    pub fn new(
        context: &Context,
        vertex_buffers: Vec<VertexBuffer>,
        indices: Indices,
        program: Program,
    ) -> Result<Self> {
        // TODO: fix unwrap
        let vao = context.create_vertex_array().unwrap();
        context.bind_vertex_array(Some(&vao));

        for VertexBuffer {
            ref buffer,
            ref binding,
        } in vertex_buffers.iter()
        {
            bind_attributes(&context, &program, buffer, binding)?;
        }

        if let Indices::IndexBuffer { ref buffer, .. } = indices {
            context.bind_buffer(GL::ELEMENT_ARRAY_BUFFER, Some(&buffer.id));
        }

        context.bind_vertex_array(None);
        context.bind_buffer(GL::ARRAY_BUFFER, None);
        context.bind_buffer(GL::ELEMENT_ARRAY_BUFFER, None);

        Ok(Self {
            context: context.clone(),
            vertex_buffers,
            indices,
            program,
            vao,
        })
    }

    pub fn set_uniform(&self, uniform: &Uniform) {
        self.program.set_uniform(&uniform);
    }

    pub fn draw(&self, uniforms: &Vec<Uniform>, instance_count: u32) -> Result<()> {
        self.draw_impl(vec![], &uniforms, None, instance_count)
    }

    pub fn draw_impl(
        &self,
        vertex_buffers: Vec<VertexBuffer>,
        uniforms: &Vec<Uniform>,
        transform_feedback: Option<&TransformFeedbackBuffer>,
        instance_count: u32,
    ) -> Result<()> {
        let context = &self.context;
        context.use_program(Some(&self.program.program));

        context.bind_vertex_array(Some(&self.vao));

        for VertexBuffer {
            ref buffer,
            binding,
        } in vertex_buffers.iter()
        {
            bind_attributes(&context, &self.program, buffer, binding)?;
        }

        for uniform in uniforms.into_iter() {
            self.set_uniform(&uniform);
        }

        if let Some(feedback_buffer) = transform_feedback {
            context.bind_transform_feedback(GL::TRANSFORM_FEEDBACK, Some(&feedback_buffer.id));
            context.bind_buffer_base(
                GL::TRANSFORM_FEEDBACK_BUFFER,
                0,
                Some(&feedback_buffer.next().id),
            );

            context.enable(GL::RASTERIZER_DISCARD);
            context.begin_transform_feedback(GL::POINTS);
        }

        let mut vertices_count: Option<usize> = None;

        for VertexBuffer { buffer, binding } in self.vertex_buffers.iter() {
            if binding.divisor > 0 {
                break;
            }
            // TODO: convert binding.size to usize
            // TODO: doesnt take into account stride
            let elements_count = buffer.size / (binding.size as usize);
            if let Some(current) = vertices_count {
                if current != elements_count {
                    vertices_count = None;
                    break;
                }
            } else {
                vertices_count = Some(elements_count);
            }
        }

        match self.indices {
            Indices::IndexBuffer {
                ref buffer,
                primitive,
            } => {
                if instance_count > 1 {
                    context.draw_elements_instanced_with_i32(
                        primitive,
                        buffer.size as i32,
                        GL::UNSIGNED_SHORT,
                        0,
                        instance_count as i32,
                    );
                } else {
                    context.draw_elements_with_i32(
                        primitive,
                        buffer.size as i32,
                        GL::UNSIGNED_SHORT,
                        0,
                    );
                }
            }

            Indices::NoIndices(primitive) => {
                let vertices_count = match vertices_count {
                    Some(count) => count,
                    None => return Err(Problem::VerticesCountMismatch),
                };

                if instance_count > 1 {
                    context.draw_arrays_instanced(
                        primitive,
                        0,
                        vertices_count as i32,
                        instance_count as i32,
                    );
                } else {
                    context.draw_arrays(primitive, 0, vertices_count as i32);
                }
            }
        }

        if transform_feedback.is_some() {
            context.end_transform_feedback();

            context.bind_buffer_base(GL::TRANSFORM_FEEDBACK_BUFFER, 0, None);
            context.bind_transform_feedback(GL::TRANSFORM_FEEDBACK, None);

            context.disable(GL::RASTERIZER_DISCARD);
        }

        context.bind_vertex_array(None);

        Ok(())
    }

    pub fn draw_to(
        &self,
        framebuffer: &Framebuffer,
        uniforms: &Vec<Uniform>,
        instance_count: u32,
    ) -> Result<()> {
        self.context
            .bind_framebuffer(GL::DRAW_FRAMEBUFFER, Some(&framebuffer.id));
        self.context
            .viewport(0, 0, framebuffer.width as i32, framebuffer.height as i32);

        self.draw_impl(vec![], &uniforms, None, instance_count)?;

        self.context.bind_framebuffer(GL::DRAW_FRAMEBUFFER, None);

        Ok(())
    }
}

pub fn bind_attributes(
    context: &Context,
    program: &Program,
    buffer: &Buffer,
    binding: &BindingInfo,
) -> Result<()> {
    context.bind_buffer(GL::ARRAY_BUFFER, Some(&buffer.id));
    let location =
        program
            .get_attrib_location(&binding.name)
            .ok_or(Problem::CannotFindAttributeBinding(
                binding.name.to_string(),
            ))?;
    context.enable_vertex_attrib_array(location);

    match binding.type_ {
        GL::FLOAT => context.vertex_attrib_pointer_with_i32(
            location,
            binding.size as i32,
            binding.type_,
            false,
            binding.stride as i32,
            binding.offset as i32,
        ),
        GL::UNSIGNED_SHORT | GL::UNSIGNED_INT | GL::INT => context
            .vertex_attrib_i_pointer_with_i32(
                location,
                binding.size as i32,
                binding.type_,
                binding.stride as i32,
                binding.offset as i32,
            ),
        _ => return Err(Problem::CannotBindUnsupportedVertexType),
    };

    context.vertex_attrib_divisor(location, binding.divisor);

    Ok(())
}

pub struct MsaaPass {
    context: Context,
    width: u32,
    height: u32,
    framebuffer: WebGlFramebuffer,
}

impl MsaaPass {
    pub fn new(context: &Context, width: u32, height: u32, samples: u32) -> Result<Self> {
        let framebuffer = context
            .create_framebuffer()
            .ok_or(Problem::CannotCreateFramebuffer)?;
        let renderbuffer = context
            .create_renderbuffer()
            .ok_or(Problem::CannotCreateRenderbuffer)?;
        context.bind_framebuffer(GL::FRAMEBUFFER, Some(&framebuffer));
        context.bind_renderbuffer(GL::RENDERBUFFER, Some(&renderbuffer));

        let mut max_samples: u32 = 0;
        if let Ok(raw_max_samples) = context.get_parameter(GL::MAX_SAMPLES) {
            max_samples = raw_max_samples.as_f64().unwrap_or(0.0) as u32;
        }

        context.renderbuffer_storage_multisample(
            GL::RENDERBUFFER,
            samples.min(max_samples) as i32,
            GL::RGBA8,
            width as i32,
            height as i32,
        );
        context.framebuffer_renderbuffer(
            GL::FRAMEBUFFER,
            GL::COLOR_ATTACHMENT0,
            GL::RENDERBUFFER,
            Some(&renderbuffer),
        );
        context.bind_framebuffer(GL::FRAMEBUFFER, None);
        context.bind_renderbuffer(GL::RENDERBUFFER, None);

        Ok(MsaaPass {
            context: context.clone(),
            width,
            height,
            framebuffer,
        })
    }

    pub fn draw_to<T>(&self, draw_call: T) -> ()
    where
        T: Fn() -> (),
    {
        let width = self.width as i32;
        let height = self.height as i32;

        self.context
            .bind_framebuffer(GL::DRAW_FRAMEBUFFER, Some(&self.framebuffer));

        // Draw stuff
        draw_call();

        self.context.bind_framebuffer(GL::DRAW_FRAMEBUFFER, None);

        self.context.disable(GL::BLEND);
        self.context
            .bind_framebuffer(GL::READ_FRAMEBUFFER, Some(&self.framebuffer));
        self.context.blit_framebuffer(
            0,
            0,
            width,
            height,
            0,
            0,
            width,
            height,
            GL::COLOR_BUFFER_BIT,
            GL::LINEAR,
        );
        self.context.bind_framebuffer(GL::READ_FRAMEBUFFER, None);
    }
}

struct TextureFormat {
    internal_format: GlDataType,
    format: GlDataType,
    type_: GlDataType,
    size: usize,
}

// https://www.khronos.org/registry/webgl/specs/latest/2.0/#TEXTURE_TYPES_FORMATS_FROM_DOM_ELEMENTS_TABLE
fn detect_texture_format(internal_format: GlDataType) -> Result<TextureFormat> {
    match internal_format {
        GL::R32F => Ok(TextureFormat {
            internal_format,
            format: GL::RED,
            type_: GL::FLOAT,
            size: 1,
        }),
        GL::RG32F => Ok(TextureFormat {
            internal_format,
            format: GL::RG,
            type_: GL::FLOAT,
            size: 2,
        }),
        GL::RGB32F => Ok(TextureFormat {
            internal_format,
            format: GL::RGB,
            type_: GL::FLOAT,
            size: 3,
        }),
        GL::RGBA32F => Ok(TextureFormat {
            internal_format,
            format: GL::RGBA,
            type_: GL::FLOAT,
            size: 4,
        }),
        _ => Err(Problem::UnsupportedTextureFormat),
    }
}
