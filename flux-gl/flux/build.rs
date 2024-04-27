use std::io::prelude::*;
use std::{env, error::Error, fs, fs::File, path::Path};

// Specify the correct GLSL version in the shaders at build time.
fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = env::var_os("OUT_DIR").expect("missing output directory");
    let target = env::var("TARGET").expect("missing compilation target");

    // TODO: rewrite the shaders in either WGSL or SPIR-V, and then compile to
    // whichever target we need.
    //
    // Specify the GLSL version.
    let version = match target.as_str() {
        "wasm32-unknown-unknown" => "300 es",

        // Below OpenGL 3.3, the GLSL and OpenGL version numbers do not match.
        // Consult the version table.
        // https://www.khronos.org/opengl/wiki/Core_Language_(GLSL)#OpenGL_and_GLSL_versions
        _ => "330 core",
    };

    for entry in fs::read_dir("shaders")? {
        let shader_file = entry?;
        let path = shader_file.path();
        let mut shader_source = File::open(&path)?;

        let mut version_shader_source = format!("#version {}\n", version);
        shader_source.read_to_string(&mut version_shader_source)?;

        let out_path = Path::new(&out_dir).join(&path);
        fs::create_dir_all(Path::new(&out_dir).join(Path::new("shaders")))?;
        fs::write(&out_path, version_shader_source.as_bytes())?;
    }

    Ok(())
}
