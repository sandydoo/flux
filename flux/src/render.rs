use crevice::std140::{self, WriteStd140};
use glow::HasContext;
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::cell::{Ref, RefCell};
use std::fmt::Write;
use std::rc::Rc;
use thiserror::Error;

pub type Context = Rc<glow::Context>;
type GlDataType = u32;
type Result<T> = std::result::Result<T, Problem>;

#[derive(Error, Debug)]
pub enum Problem {
    #[error("Ran out of memory")]
    OutOfMemory,

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
        None => "Cannot create shader".to_string(),
    })]
    CannotCreateShader(Option<String>),

    #[error("Cannot create program")]
    CannotCreateProgram,

    #[error("Cannot link program: {0}")]
    CannotLinkProgram(String),

    #[error("One of the framebuffers is incomplete")]
    FramebufferIncomplete,

    #[error("Cannot write to texture")]
    CannotWriteToTexture,

    #[error("Unexpected data size. Expected: {expected:?}. Actual: {actual:?} ")]
    WrongDataSize { expected: usize, actual: usize },

    #[error("Cannot write to texture")]
    UnsupportedTextureFormat,

    #[error("Vertex attribute type is not supported")]
    CannotBindUnsupportedVertexType,
}

#[derive(Clone, Debug)]
pub struct Buffer {
    context: Context,
    pub id: glow::Buffer,
    pub size: usize,
    pub type_: u32,
    pub usage: u32,
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            self.context.delete_buffer(self.id);
        }
    }
}

#[allow(dead_code)]
impl Buffer {
    pub fn from_bytes(
        context: &Context,
        data: &[u8],
        buffer_type: u32,
        usage: u32,
    ) -> Result<Self> {
        let buffer = unsafe {
            let buffer = context
                .create_buffer()
                .map_err(|_| Problem::CannotCreateBuffer)?;

            context.bind_buffer(buffer_type, Some(buffer));
            context.buffer_data_u8_slice(buffer_type, data, usage);
            context.bind_buffer(buffer_type, None);

            buffer
        };

        Ok(Self {
            context: Rc::clone(context),
            id: buffer,
            size: data.len(),
            type_: buffer_type,
            usage,
        })
    }

    pub fn from_f32(context: &Context, data: &[f32], buffer_type: u32, usage: u32) -> Result<Self> {
        Self::from_bytes(context, bytemuck::cast_slice(data), buffer_type, usage)
    }

    pub fn from_u16(context: &Context, data: &[u16], buffer_type: u32, usage: u32) -> Result<Self> {
        Self::from_bytes(context, bytemuck::cast_slice(data), buffer_type, usage)
    }

    pub fn update(&self, data: &[u8]) {
        unsafe {
            self.context.bind_buffer(self.type_, Some(self.id));
            self.context.buffer_sub_data_u8_slice(self.type_, 0, data);
            self.context.bind_buffer(self.type_, None);
        }
    }

    pub fn overwrite(&mut self, data: &[u8]) {
        unsafe {
            self.context.bind_buffer(self.type_, Some(self.id));
            self.context
                .buffer_data_u8_slice(self.type_, data, self.usage);
            self.context.bind_buffer(self.type_, None);
        }
        self.size = data.len();
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
            mag_filter: glow::NEAREST,
            min_filter: glow::NEAREST,
            wrap_s: glow::CLAMP_TO_EDGE,
            wrap_t: glow::CLAMP_TO_EDGE,
            format: glow::RGBA32F,
        }
    }
}

#[derive(Clone)]
pub struct Framebuffer {
    context: Context,
    pub id: glow::Framebuffer,
    pub width: u32,
    pub height: u32,
    pub texture: glow::Texture,
    pub options: TextureOptions,
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        unsafe {
            self.context
                .bind_framebuffer(glow::FRAMEBUFFER, Some(self.id));
            self.context.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                None,
                0,
            );
            self.context.bind_framebuffer(glow::FRAMEBUFFER, None);
            self.context.delete_framebuffer(self.id);
            self.context.delete_texture(self.texture);
        }
    }
}

impl Framebuffer {
    pub fn new(
        context: &Context,
        width: u32,
        height: u32,
        options: TextureOptions,
    ) -> Result<Self> {
        Self::with_params(context, width, height, options)
    }

    fn with_params(
        context: &Context,
        width: u32,
        height: u32,
        options: TextureOptions,
    ) -> Result<Self> {
        let (framebuffer, texture) = unsafe {
            let texture = context
                .create_texture()
                .map_err(|_| Problem::CannotCreateTexture)?;

            context.bind_texture(glow::TEXTURE_2D, Some(texture));
            context.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                options.mag_filter as i32,
            );
            context.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                options.min_filter as i32,
            );
            context.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                options.wrap_s as i32,
            );
            context.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                options.wrap_t as i32,
            );
            context.bind_texture(glow::TEXTURE_2D, None);

            let framebuffer = context
                .create_framebuffer()
                .map_err(|_| Problem::CannotCreateFramebuffer)?;

            (framebuffer, texture)
        };

        Ok(Self {
            context: Rc::clone(context),
            id: framebuffer,
            width,
            height,
            texture,
            options,
        })
    }

    pub fn with_data<T: bytemuck::Pod>(&self, data: Option<&[T]>) -> Result<()> {
        let TextureFormat {
            internal_format,
            format,
            type_,
            size,
        } = detect_texture_format(self.options.format)?;

        let expected_size = size * ((self.width * self.height) as usize);
        if let Some(buffer) = data {
            if buffer.len() != expected_size {
                return Err(Problem::WrongDataSize {
                    expected: expected_size,
                    actual: buffer.len(),
                });
            }
        }

        unsafe {
            self.context
                .bind_texture(glow::TEXTURE_2D, Some(self.texture));

            self.context.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                internal_format as i32,
                self.width as i32,
                self.height as i32,
                0,
                format,
                type_,
                data.map(|buffer| bytemuck::cast_slice(buffer)),
            );
            // .map_err(|Err(Problem::CannotWriteToTexture))?;

            self.context.bind_texture(glow::TEXTURE_2D, None);

            self.context
                .bind_framebuffer(glow::FRAMEBUFFER, Some(self.id));
            self.context.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(self.texture),
                0,
            );

            if self.context.check_framebuffer_status(glow::FRAMEBUFFER)
                != glow::FRAMEBUFFER_COMPLETE
            {
                return Err(Problem::FramebufferIncomplete);
            }

            self.context.bind_framebuffer(glow::FRAMEBUFFER, None);
        }

        Ok(())
    }

    pub fn draw_to<T>(&self, context: &Context, draw_call: T)
    where
        T: Fn(),
    {
        unsafe {
            context.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(self.id));
            context.viewport(0, 0, self.width as i32, self.height as i32);
            draw_call();
            context.bind_framebuffer(glow::DRAW_FRAMEBUFFER, None);
        }
    }

    pub fn blit_to(&self, context: &Context, target_framebuffer: &Framebuffer) {
        unsafe {
            context.disable(glow::BLEND);
            context.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(target_framebuffer.id));
            context.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(self.id));
            context.blit_framebuffer(
                0,
                0,
                self.width as i32,
                self.height as i32,
                0,
                0,
                target_framebuffer.width as i32,
                target_framebuffer.height as i32,
                glow::COLOR_BUFFER_BIT,
                glow::LINEAR,
            );
            context.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
            context.bind_framebuffer(glow::DRAW_FRAMEBUFFER, None);
        }
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
        let front = Framebuffer::new(context, width, height, options)?;
        let back = Framebuffer::new(context, width, height, options)?;
        Ok(Self {
            width,
            height,
            front: RefCell::new(front),
            back: RefCell::new(back),
        })
    }

    pub fn with_data<T: bytemuck::Pod>(&self, data: Option<&[T]>) -> Result<()> {
        self.front.borrow().with_data(data)?;
        self.back.borrow().with_data(data)?;

        Ok(())
    }

    pub fn current(&self) -> Ref<Framebuffer> {
        self.front.borrow()
    }

    pub fn next(&self) -> Ref<Framebuffer> {
        self.back.borrow()
    }

    pub fn swap(&self) {
        self.front.swap(&self.back);
    }

    pub fn draw_to<T>(&self, context: &Context, draw_call: T)
    where
        T: Fn(&Framebuffer),
    {
        let framebuffer = self.next();

        unsafe {
            context.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(framebuffer.id));
            context.viewport(0, 0, framebuffer.width as i32, framebuffer.height as i32);
            draw_call(&self.current());
            context.bind_framebuffer(glow::DRAW_FRAMEBUFFER, None);
        }

        drop(framebuffer);
        self.swap();
    }

    pub fn blit_to(&self, context: &Context, target_framebuffer: &DoubleFramebuffer) {
        self.current()
            .blit_to(context, &target_framebuffer.current());
    }
}

pub struct TransformFeedback {
    context: Context,
    pub feedback: glow::TransformFeedback,
    pub buffer: Buffer,

    // Transform feedback requires a backing store. In case we’re in a headless
    // environment, just bind a 1x1 renderbuffer and don’t worry about it.
    framebuffer: glow::Framebuffer,
    renderbuffer: glow::Renderbuffer,
}

impl Drop for TransformFeedback {
    fn drop(&mut self) {
        unsafe {
            self.context
                .bind_transform_feedback(glow::TRANSFORM_FEEDBACK, Some(self.feedback));
            self.context
                .bind_buffer_base(glow::TRANSFORM_FEEDBACK_BUFFER, 0, None);
            self.context
                .bind_transform_feedback(glow::TRANSFORM_FEEDBACK, None);
            self.context.delete_transform_feedback(self.feedback);

            self.context.delete_framebuffer(self.framebuffer);
            self.context.delete_renderbuffer(self.renderbuffer);
        }
    }
}

impl TransformFeedback {
    pub fn new(context: &Context, data: &[u8]) -> Result<Self> {
        let feedback = unsafe {
            context
                .create_transform_feedback()
                .map_err(|_| Problem::OutOfMemory)?
        };
        let buffer = Buffer::from_bytes(context, data, glow::ARRAY_BUFFER, glow::DYNAMIC_COPY)?;

        unsafe {
            context.bind_transform_feedback(glow::TRANSFORM_FEEDBACK, Some(feedback));
            context.bind_buffer_base(glow::TRANSFORM_FEEDBACK_BUFFER, 0, Some(buffer.id));
            context.bind_transform_feedback(glow::TRANSFORM_FEEDBACK, None);
        }

        let (renderbuffer, framebuffer) = unsafe {
            let renderbuffer = context
                .create_renderbuffer()
                .map_err(|_| Problem::CannotCreateRenderbuffer)?;
            context.bind_renderbuffer(glow::RENDERBUFFER, Some(renderbuffer));
            context.renderbuffer_storage(glow::RENDERBUFFER, glow::RGB8, 1, 1);
            let framebuffer = context
                .create_framebuffer()
                .map_err(|_| Problem::CannotCreateFramebuffer)?;
            context.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(framebuffer));
            context.framebuffer_renderbuffer(
                glow::DRAW_FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::RENDERBUFFER,
                Some(renderbuffer),
            );

            if context.check_framebuffer_status(glow::DRAW_FRAMEBUFFER)
                != glow::FRAMEBUFFER_COMPLETE
            {
                return Err(Problem::FramebufferIncomplete);
            }

            context.bind_framebuffer(glow::DRAW_FRAMEBUFFER, None);
            context.bind_renderbuffer(glow::RENDERBUFFER, None);

            (renderbuffer, framebuffer)
        };

        Ok(Self {
            context: Rc::clone(context),
            feedback,
            buffer,
            framebuffer,
            renderbuffer,
        })
    }

    pub fn overwrite_buffer(&mut self, data: &[u8]) -> Result<()> {
        self.buffer.overwrite(data);

        Ok(())
    }

    pub fn draw_to<T>(&self, draw_call: T)
    where
        T: Fn(),
    {
        unsafe {
            self.context
                .bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(self.framebuffer));
            self.context
                .bind_transform_feedback(glow::TRANSFORM_FEEDBACK, Some(self.feedback));

            self.context.viewport(0, 0, 1, 1);
            self.context.enable(glow::RASTERIZER_DISCARD);
            self.context.begin_transform_feedback(glow::POINTS);

            draw_call();

            self.context.end_transform_feedback();
            self.context
                .bind_transform_feedback(glow::TRANSFORM_FEEDBACK, None);
            self.context.disable(glow::RASTERIZER_DISCARD);
            self.context.bind_framebuffer(glow::DRAW_FRAMEBUFFER, None);
        }
    }
}

pub struct DoubleTransformFeedback {
    pub active_buffer: usize,
    buffers: [TransformFeedback; 2],
}

impl DoubleTransformFeedback {
    pub fn new(context: &Context, data: &[u8]) -> Result<Self> {
        let front = TransformFeedback::new(context, data)?;
        let back = TransformFeedback::new(context, data)?;

        Ok(Self {
            active_buffer: 0,
            buffers: [front, back],
        })
    }

    pub fn overwrite_buffer(&mut self, data: &[u8]) -> Result<()> {
        self.buffers[0].overwrite_buffer(data)?;
        self.buffers[1].overwrite_buffer(data)?;
        Ok(())
    }

    pub fn current_buffer(&self) -> &TransformFeedback {
        &self.buffers[self.active_buffer]
    }

    pub fn next_buffer(&self) -> &TransformFeedback {
        &self.buffers[1 - self.active_buffer]
    }

    pub fn swap(&mut self) {
        self.active_buffer = 1 - self.active_buffer;
    }

    pub fn draw_to<T>(&mut self, draw_call: T)
    where
        T: Fn(),
    {
        self.next_buffer().draw_to(draw_call);
        self.swap();
    }
}

#[derive(Clone)]
pub struct Program {
    context: Context,
    pub program: glow::Program,
    attributes: FxHashMap<String, AttributeInfo>,
    uniforms: FxHashMap<String, UniformInfo>,
}

impl Drop for Program {
    fn drop(&mut self) {
        unsafe {
            self.context.delete_program(self.program);
        }
    }
}

impl Program {
    pub fn new(context: &Context, shaders: (&str, &str)) -> Result<Self> {
        Self::new_impl(context, shaders, None, None)
    }

    pub fn new_with_transform_feedback(
        context: &Context,
        shaders: (&str, &str),
        transform_feedback: &TransformFeedbackInfo,
    ) -> Result<Self> {
        Self::new_impl(context, shaders, None, Some(transform_feedback))
    }

    pub fn new_with_variables(
        context: &Context,
        shaders: (&str, &str),
        variables: &[(&'static str, &str)],
    ) -> Result<Self> {
        Self::new_impl(context, shaders, Some(variables), None)
    }

    pub fn new_impl(
        context: &Context,
        shaders: (&str, &str),
        optional_variables: Option<&[(&'static str, &str)]>,
        transform_feedback: Option<&TransformFeedbackInfo>,
    ) -> Result<Self> {
        let vertex_shader = compile_shader(
            context,
            glow::VERTEX_SHADER,
            &preprocess_shader(shaders.0, optional_variables),
        )?;
        let fragment_shader = compile_shader(
            context,
            glow::FRAGMENT_SHADER,
            &preprocess_shader(shaders.1, optional_variables),
        )?;

        let program = unsafe {
            let program = context
                .create_program()
                .map_err(|_| Problem::CannotCreateProgram)?;
            context.attach_shader(program, vertex_shader);
            context.attach_shader(program, fragment_shader);

            if let Some(TransformFeedbackInfo { names, mode }) = transform_feedback {
                context.transform_feedback_varyings(program, names, *mode);
            }

            context.link_program(program);

            if !context.get_program_link_status(program) {
                return Err(Problem::CannotLinkProgram(
                    context.get_program_info_log(program),
                ));
            }

            // Delete the shaders to free up memory
            context.detach_shader(program, vertex_shader);
            context.detach_shader(program, fragment_shader);
            context.delete_shader(vertex_shader);
            context.delete_shader(fragment_shader);

            program
        };

        // Get attribute locations
        let mut attributes = FxHashMap::default();
        unsafe {
            let attribute_count = context.get_active_attributes(program);
            for num in 0..attribute_count {
                if let Some(info) = context.get_active_attribute(program, num) {
                    if let Some(location) = context.get_attrib_location(program, &info.name) {
                        attributes.insert(
                            info.name,
                            AttributeInfo {
                                type_: info.atype,
                                size: info.size as u32,
                                location,
                            },
                        );
                    }
                }
            }
        }

        // Get uniform locations
        let mut uniforms = FxHashMap::default();
        unsafe {
            let uniform_count = context.get_active_uniforms(program);
            for num in 0..uniform_count {
                if let Some(info) = context.get_active_uniform(program, num) {
                    if let Some(location) = context.get_uniform_location(program, &info.name) {
                        uniforms.insert(
                            info.name,
                            UniformInfo {
                                type_: info.utype,
                                size: info.size,
                                location,
                            },
                        );
                    }
                }
            }
        }

        Ok(Program {
            context: Rc::clone(context),
            program,
            attributes,
            uniforms,
        })
    }

    pub fn use_program(&self) {
        unsafe {
            self.context.use_program(Some(self.program));
        }
    }

    pub fn set_uniforms(&self, uniforms: &[&Uniform]) {
        for uniform in uniforms.iter() {
            self.set_uniform(uniform);
        }
    }

    pub fn set_uniform(&self, uniform: &Uniform) {
        let context = &self.context;
        self.use_program();

        unsafe {
            match uniform.value {
                UniformValue::UnsignedInt(value) => {
                    context.uniform_1_u32(self.get_uniform_location(uniform.name).as_ref(), value)
                }

                UniformValue::SignedInt(value) => {
                    context.uniform_1_i32(self.get_uniform_location(uniform.name).as_ref(), value)
                }

                UniformValue::Float(value) => {
                    context.uniform_1_f32(self.get_uniform_location(uniform.name).as_ref(), value)
                }

                UniformValue::Vec2(value) => context.uniform_2_f32(
                    self.get_uniform_location(uniform.name).as_ref(),
                    value[0],
                    value[1],
                ),

                UniformValue::Vec3(value) => context.uniform_3_f32(
                    self.get_uniform_location(uniform.name).as_ref(),
                    value[0],
                    value[1],
                    value[2],
                ),

                UniformValue::Vec3Array(value) => context
                    .uniform_3_f32_slice(self.get_uniform_location(uniform.name).as_ref(), value),

                UniformValue::Vec4Array(value) => context
                    .uniform_4_f32_slice(self.get_uniform_location(uniform.name).as_ref(), value),

                UniformValue::Mat4(value) => context.uniform_matrix_4_f32_slice(
                    self.get_uniform_location(uniform.name).as_ref(),
                    false,
                    value,
                ),

                UniformValue::Texture2D(id) => {
                    context
                        .uniform_1_i32(self.get_uniform_location(uniform.name).as_ref(), id as i32);
                }
            }
        }
    }

    pub fn set_uniform_block(&self, name: &str, index: u32) {
        if let Some(location) = self.get_uniform_block_location(name) {
            unsafe {
                self.context
                    .uniform_block_binding(self.program, location, index);
            }
        }
        // TODO return an error here?
    }

    pub fn get_attrib_location(&self, name: &str) -> Option<u32> {
        self.attributes.get(name).map(|info| info.location)
    }

    pub fn get_uniform_location(&self, name: &str) -> Option<glow::UniformLocation> {
        self.uniforms.get(name).map(|info| info.location.clone())
    }

    pub fn get_uniform_block_location(&self, name: &str) -> Option<u32> {
        unsafe { self.context.get_uniform_block_index(self.program, name) }
    }
}

fn preprocess_shader<'a>(
    source: &'a str,
    optional_variables: Option<&[(&'static str, &str)]>,
) -> Cow<'a, str> {
    if let Some(variables) = optional_variables {
        let mut preamble = String::new();
        for (name, value) in variables.iter() {
            writeln!(&mut preamble, "#define {} {}", name, value).unwrap();
        }

        if source.starts_with("#version") {
            let (version, source_rest) = source.split_once('\n').unwrap();
            format!("{}\n{}{}", version, preamble, source_rest).into()
        } else {
            (preamble + source).into()
        }
    } else {
        source.into()
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct AttributeInfo {
    type_: u32,
    size: u32,
    location: u32,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct UniformInfo {
    type_: u32,
    size: i32,
    location: glow::UniformLocation,
}

#[derive(Default)]
pub struct Attribute {
    pub location: Option<u32>,
    pub data_type: GlDataType,
    pub divisor: u32,
}

pub struct TransformFeedbackInfo<'a> {
    pub names: &'a [&'static str],
    pub mode: u32,
}

pub struct Uniform<'a> {
    pub name: &'static str,
    pub value: UniformValue<'a>,
}

#[allow(dead_code)]
#[derive(Clone)]
pub enum UniformValue<'a> {
    SignedInt(i32),
    UnsignedInt(u32),
    Float(f32),
    Vec2(&'a [f32; 2]),
    Vec3(&'a [f32; 3]),
    // TODO: use nalgebra types here
    Vec3Array(&'a [f32]),
    Vec4Array(&'a [f32]),
    Mat4(&'a [f32]),
    Texture2D(u32),
}

pub fn compile_shader(context: &Context, shader_type: u32, source: &str) -> Result<glow::Shader> {
    unsafe {
        let shader = context
            .create_shader(shader_type)
            .map_err(|_| Problem::CannotCreateShader(None))?;
        context.shader_source(shader, source);
        context.compile_shader(shader);

        if context.get_shader_compile_status(shader) {
            Ok(shader)
        } else {
            Err(Problem::CannotCreateShader(Some(
                context.get_shader_info_log(shader),
            )))
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct VertexBufferLayout {
    pub name: &'static str,
    pub size: u32,
    pub type_: u32,
    pub divisor: u32,
    pub stride: u32,
    pub offset: u32,
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
        glow::RGB | glow::RGB8 | glow::SRGB8 => Ok(TextureFormat {
            internal_format,
            format: glow::RGB,
            type_: glow::UNSIGNED_BYTE,
            size: 3,
        }),
        glow::RGBA | glow::RGBA8 => Ok(TextureFormat {
            internal_format,
            format: glow::RGBA,
            type_: glow::UNSIGNED_BYTE,
            size: 4,
        }),
        glow::R16F => Ok(TextureFormat {
            internal_format,
            format: glow::RED,
            type_: glow::HALF_FLOAT,
            size: 1,
        }),
        glow::R32F => Ok(TextureFormat {
            internal_format,
            format: glow::RED,
            type_: glow::FLOAT,
            size: 1,
        }),
        glow::RG16F => Ok(TextureFormat {
            internal_format,
            format: glow::RG,
            type_: glow::HALF_FLOAT,
            size: 2,
        }),
        glow::RG32F => Ok(TextureFormat {
            internal_format,
            format: glow::RG,
            type_: glow::FLOAT,
            size: 2,
        }),
        glow::RGB32F => Ok(TextureFormat {
            internal_format,
            format: glow::RGB,
            type_: glow::FLOAT,
            size: 3,
        }),
        glow::RGBA32F => Ok(TextureFormat {
            internal_format,
            format: glow::RGBA,
            type_: glow::FLOAT,
            size: 4,
        }),
        _ => Err(Problem::UnsupportedTextureFormat),
    }
}

// Type hackery to implement Std140 for Vec
pub struct UniformArray<T>(pub Vec<T>);

impl<T> WriteStd140 for UniformArray<T>
where
    T: WriteStd140,
{
    fn write_std140<W: std::io::Write>(
        &self,
        writer: &mut std140::Writer<W>,
    ) -> std::io::Result<usize> {
        writer.write(self.0.as_slice())
    }
}

pub struct UniformBlock<T> {
    context: Context,
    pub index: u32,
    pub data: T,
    pub buffer: Buffer,
}

impl<T> UniformBlock<T>
where
    T: WriteStd140,
{
    pub fn new(context: &Context, data: T, index: u32, usage: GlDataType) -> Result<Self> {
        let buffer = Buffer::from_bytes(
            context,
            &new_std140_buffer(&data),
            glow::ARRAY_BUFFER,
            usage,
        )?;

        Ok(Self {
            context: Rc::clone(context),
            index,
            data,
            buffer,
        })
    }

    pub fn update<F>(&mut self, update_closure: F) -> &Self
    where
        F: Fn(&mut T),
    {
        update_closure(&mut self.data);
        self
    }

    pub fn buffer_data(&self) {
        self.buffer.update(&new_std140_buffer(&self.data));
    }

    pub fn bind(&self) {
        unsafe {
            self.context
                .bind_buffer_base(glow::UNIFORM_BUFFER, self.index, Some(self.buffer.id));
        }
    }
}

fn new_std140_buffer<T>(data: &T) -> Vec<u8>
where
    T: WriteStd140,
{
    let mut buffer = Vec::new();
    let mut writer = std140::Writer::new(&mut buffer);
    writer.write(data).unwrap();

    buffer
}

pub struct VertexArrayObject {
    context: Context,
    pub id: glow::VertexArray,
}

impl Drop for VertexArrayObject {
    fn drop(&mut self) {
        unsafe {
            self.context.delete_vertex_array(self.id);
        }
    }
}

impl VertexArrayObject {
    pub fn empty(context: &Context) -> Result<Self> {
        let id = unsafe {
            context
                .create_vertex_array()
                .map_err(|_| Problem::OutOfMemory)?
        };

        Ok(Self {
            id,
            context: Rc::clone(context),
        })
    }

    pub fn new(
        context: &Context,
        program: &Program,
        vertices: &[(&Buffer, VertexBufferLayout)],
        indices: Option<&Buffer>,
    ) -> Result<Self> {
        let vao = Self::empty(context)?;
        vao.update(program, vertices, indices)?;
        Ok(vao)
    }

    pub fn update(
        &self,
        program: &Program,
        vertices: &[(&Buffer, VertexBufferLayout)],
        indices: Option<&Buffer>,
    ) -> Result<()> {
        unsafe {
            self.context.bind_vertex_array(Some(self.id));

            for (vertex, attribute) in vertices.iter() {
                bind_attributes(&self.context, program, vertex, attribute)?;
            }

            if indices.is_some() {
                self.context
                    .bind_buffer(glow::ELEMENT_ARRAY_BUFFER, indices.map(|buffer| buffer.id));
            }

            self.context.bind_vertex_array(None);
        }

        Ok(())
    }

    pub fn bind(&self) {
        unsafe {
            self.context.bind_vertex_array(Some(self.id));
        }
    }
}

pub fn bind_attributes(
    context: &Context,
    program: &Program,
    buffer: &Buffer,
    buffer_layout: &VertexBufferLayout,
) -> Result<()> {
    unsafe {
        context.bind_buffer(glow::ARRAY_BUFFER, Some(buffer.id));

        if let Some(location) = program.get_attrib_location(buffer_layout.name) {
            context.enable_vertex_attrib_array(location);

            match buffer_layout.type_ {
                glow::FLOAT => context.vertex_attrib_pointer_f32(
                    location,
                    buffer_layout.size as i32,
                    buffer_layout.type_,
                    false,
                    buffer_layout.stride as i32,
                    buffer_layout.offset as i32,
                ),
                glow::UNSIGNED_SHORT | glow::UNSIGNED_INT | glow::INT => context
                    .vertex_attrib_pointer_i32(
                        location,
                        buffer_layout.size as i32,
                        buffer_layout.type_,
                        buffer_layout.stride as i32,
                        buffer_layout.offset as i32,
                    ),
                _ => return Err(Problem::CannotBindUnsupportedVertexType),
            };

            context.vertex_attrib_divisor(location, buffer_layout.divisor);
        }

        context.bind_buffer(glow::ARRAY_BUFFER, None);
    }

    Ok(())
}
