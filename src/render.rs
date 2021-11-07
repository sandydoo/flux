use std::cell::{Ref, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;
use thiserror::Error;

use js_sys::WebAssembly;
use wasm_bindgen::JsCast;
use web_sys::{
    WebGl2RenderingContext as GL, WebGlBuffer, WebGlFramebuffer, WebGlProgram, WebGlShader,
    WebGlTexture, WebGlUniformLocation, WebGlVertexArrayObject,
};

pub type Context = Rc<GL>;
type GlDataType = u32;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Error, Debug)]
enum Problem {
    CannotCreateTexture(),
    CannotCreateFramebuffer(),
    CannotCreateShader(Option<String>),
    CannotCreateProgram(),
    CannotWriteToTexture(),
    WrongDataType(),
    AttribNotActive(String),
}

impl fmt::Display for Problem {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let desc = match self {
            Problem::CannotCreateTexture() => "Cannot create texture",
            Problem::CannotCreateFramebuffer() => "Cannot create framebuffer",
            Problem::CannotCreateShader(maybe_desc) => &maybe_desc.as_ref().unwrap(),
            Problem::AttribNotActive(name) => "Attribute not active",
            // TODO: fix
            _ => "Something went wrong",
        };
        fmt.write_str(desc)
    }
}

#[derive(Clone, Debug)]
pub struct Buffer<T> {
    context: Context,
    id: WebGlBuffer,
    size: usize,
    type_: u32,
    marker: PhantomData<T>,
}

impl Buffer<f32> {
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

        let buffer = context.create_buffer().ok_or("failed to create buffer")?;

        context.bind_buffer(buffer_type, Some(&buffer));
        context.buffer_data_with_array_buffer_view(buffer_type, &data_array, usage);
        context.bind_buffer(buffer_type, None);

        Ok(Self {
            context: context.clone(),
            id: buffer,
            size: data.len(),
            type_: buffer_type,
            marker: PhantomData,
        })
    }
}

impl Buffer<u16> {
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

        let buffer = context.create_buffer().ok_or("failed to create buffer")?;

        context.bind_buffer(buffer_type, Some(&buffer));
        context.buffer_data_with_array_buffer_view(buffer_type, &data_array, usage);
        context.bind_buffer(buffer_type, None);

        Ok(Self {
            context: context.clone(),
            id: buffer,
            size: data.len(),
            type_: buffer_type,
            marker: PhantomData,
        })
    }
}

#[derive(Clone, Copy)]
pub struct TextureOptions {
    mag_filter: GlDataType,
    min_filter: GlDataType,
    wrap_s: GlDataType,
    wrap_t: GlDataType,
}

impl Default for TextureOptions {
    fn default() -> Self {
        TextureOptions {
            mag_filter: GL::LINEAR,
            min_filter: GL::LINEAR,
            wrap_s: GL::CLAMP_TO_EDGE,
            wrap_t: GL::CLAMP_TO_EDGE,
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
            .ok_or(Problem::CannotCreateTexture())?;

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
            .ok_or(Problem::CannotCreateFramebuffer())?;

        Ok(Self {
            context: context.clone(),
            id: framebuffer,
            width,
            height,
            texture,
        })
    }

    pub fn with_f32_data(self, data: &Vec<f32>) -> Result<Self> {
        self.context
            .bind_texture(GL::TEXTURE_2D, Some(&self.texture));
        unsafe {
            let array = js_sys::Float32Array::view(data);
            self.context.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_array_buffer_view(
                GL::TEXTURE_2D,
                0,
                GL::RGBA32F as i32,
                self.width as i32,
                self.height as i32,
                0,
                GL::RGBA,
                GL::FLOAT,
                Some(&array),
            ).or(Err(Box::new(Problem::CannotWriteToTexture())))?;
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
}

pub struct DoubleFramebuffer {
    context: Context,
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
            context: context.clone(),
            width,
            height,
            front: RefCell::new(front),
            back: RefCell::new(back),
        })
    }

    pub fn with_f32_data(self, data: &Vec<f32>) -> Result<Self> {
        if data.len() != (self.width * self.height * 4) as usize {
            return Err(Box::new(Problem::WrongDataType()));
        }

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

pub struct Program {
    context: Context,
    program: WebGlProgram,
    attributes: HashMap<String, AttributeInfo>,
    uniforms: HashMap<String, UniformInfo>,
}

impl Program {
    pub fn new(context: &Context, shaders: (&str, &str)) -> Result<Program> {
        let vertex_shader = compile_shader(&context, GL::VERTEX_SHADER, shaders.0)?;
        let fragment_shader = compile_shader(&context, GL::FRAGMENT_SHADER, shaders.1)?;

        let program = context
            .create_program()
            .ok_or(Problem::CannotCreateProgram())?;
        context.attach_shader(&program, &vertex_shader);
        context.attach_shader(&program, &fragment_shader);
        context.link_program(&program);

        // Delete the shaders to free up memory
        context.detach_shader(&program, &vertex_shader);
        context.detach_shader(&program, &fragment_shader);
        context.delete_shader(Some(&vertex_shader));
        context.delete_shader(Some(&fragment_shader));

        // Get attribute locations
        let mut attributes = HashMap::new();
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
        let mut uniforms = HashMap::new();
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
    pub fn set_uniform(&self, uniform: &Uniform<'_>) {
        let context = &self.context;
        context.use_program(Some(&self.program));

        match uniform.value {
            UniformValue::Float(value) => {
                context.uniform1f(self.get_uniform_location(&uniform.name).as_ref(), value)
            }

            UniformValue::Texture2D(texture, id) => {
                context.active_texture(GL::TEXTURE0 + id);
                context.bind_texture(GL::TEXTURE_2D, Some(&texture));
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

struct AttributeInfo {
    type_: u32,
    size: u32,
    location: u32,
}

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

pub enum AttributeType {
    SignedInt(),
    UnsignedInt(),
    Float(),
}

impl AttributeType {
    pub fn to_gl_type(&self) -> GlDataType {
        match self {
            Self::SignedInt() => GL::INT,
            Self::UnsignedInt() => GL::UNSIGNED_INT,
            Self::Float() => GL::FLOAT,
        }
    }
}

pub enum AttributeValue {
    SignedInt(i32),
    Float(f32),
}

pub enum UniformValue<'a> {
    // SignedInt(i32),
    // UnsignedInt(u32),
    Float(f32),
    Texture2D(&'a WebGlTexture, u32),
}

pub enum UniformType {
    // SignedInt(),
    // UnsignedInt(),
    Float(),
    Texture2D(),
}

impl UniformType {
    pub fn to_gl_type(&self) -> GlDataType {
        match self {
            // Self::SignedInt() => GL::INT,
            // Self::UnsignedInt() => GL::UNSIGNED_INT,
            Self::Float() => GL::FLOAT,
            Self::Texture2D() => GL::TEXTURE_2D,
        }
    }
}

pub struct Uniform<'a> {
    pub name: String,
    pub value: UniformValue<'a>,
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
        Err(Box::new(Problem::CannotCreateShader(Some(
            context.get_shader_info_log(&shader).unwrap(),
        ))))
        // Err(Box::new(context
        //     .get_shader_info_log(&shader)
        //     .unwrap())
    }
}

pub fn link_program(
    context: &GL,
    vertex_shader: &WebGlShader,
    fragment_shader: &WebGlShader,
) -> Result<WebGlProgram> {
    let program = context
        .create_program()
        .ok_or_else(|| String::from("Unable to create shader object"))?;

    context.attach_shader(&program, vertex_shader);
    context.attach_shader(&program, fragment_shader);
    context.link_program(&program);

    if context
        .get_program_parameter(&program, GL::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        Err(Box::new(Problem::CannotCreateProgram()))
        // Err(context
        //     .get_program_info_log(&program)
        //     .unwrap_or_else(|| String::from("Unknown error creating program object")))
    }
}

pub struct VertexBuffer<T> {
    pub buffer: Buffer<T>,
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
    IndexBuffer { buffer: Buffer<u16>, primitive: u32 },
    NoIndices(u32),
}

pub struct RenderPass {
    context: Context,
    vertex_buffers: Vec<VertexBuffer<f32>>,
    indices: Indices,
    program: Program,
    vao: WebGlVertexArrayObject,
}

impl RenderPass {
    pub fn new(
        context: &Context,
        vertex_buffers: Vec<VertexBuffer<f32>>,
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
            context.bind_buffer(GL::ARRAY_BUFFER, Some(&buffer.id));
            // TODO: fix unwrap
            let location = program.get_attrib_location(&binding.name).unwrap();
            context.enable_vertex_attrib_array(location);
            context.vertex_attrib_pointer_with_i32(
                location,
                binding.size as i32,
                binding.type_,
                false,
                binding.offset as i32,
                binding.stride as i32,
            );
            context.vertex_attrib_divisor(location, binding.divisor);
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

    pub fn draw(&self, uniforms: Vec<Uniform<'_>>, instance_count: u32) -> Result<()> {
        let context = &self.context;
        context.use_program(Some(&self.program.program));
        context.bind_vertex_array(Some(&self.vao));

        for uniform in uniforms.into_iter() {
            self.program.set_uniform(&uniform);
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
                // TODO: fix
                context.draw_arrays(primitive, 0, 20 * 20);
            }
        }

        context.bind_vertex_array(None);

        Ok(())
    }

    pub fn draw_to(
        &self,
        framebuffer: &Framebuffer,
        uniforms: Vec<Uniform<'_>>,
        instance_count: u32,
    ) -> Result<()> {
        let previous_viewport: Vec<i32> = self
            .context
            .get_parameter(GL::VIEWPORT)
            .unwrap()
            .dyn_into::<js_sys::Int32Array>()
            .unwrap()
            .to_vec();

        self.context
            .bind_framebuffer(GL::FRAMEBUFFER, Some(&framebuffer.id));
        self.context
            .viewport(0, 0, framebuffer.width as i32, framebuffer.height as i32);

        self.draw(uniforms, instance_count)?;

        self.context.bind_framebuffer(GL::FRAMEBUFFER, None);

        self.context.viewport(
            previous_viewport[0] as i32,
            previous_viewport[1] as i32,
            previous_viewport[2] as i32,
            previous_viewport[3] as i32,
        );

        Ok(())
    }
}
