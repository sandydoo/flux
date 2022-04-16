use std::io::prelude::*;
use std::{env, error::Error, fs, fs::File, path::Path};

// Specify the correct GLSL version in the shaders at build time.
fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let target = env::var("TARGET").unwrap();
    let shaders_files = fs::read_dir("shaders")?;

    // TODO: rewrite the shaders to either WGSL or SPIR-V, and the compile to
    // whichever target we need.
    let version = match target.as_str() {
        "wasm32-unknown-unknown" => "300 es",
        "aarch64-apple-darwin" | "x86_64-apple-darwin" => "330",
        _ => "460", // `precision sampler2D` was added to core much later
    };

    for shader_file in shaders_files {
        if let Ok(entry) = shader_file {
            let path = entry.path();
            let mut shader_source = File::open(&path)?;

            let mut version_shader_source = format!("#version {}\n", version);
            shader_source.read_to_string(&mut version_shader_source)?;

            let dest_path = Path::new(&out_dir).join(&path);
            fs::create_dir_all(Path::new(&out_dir).join(Path::new("shaders")))?;
            fs::write(&dest_path, version_shader_source.as_bytes())?;
        }
    }

    Ok(())
}
