use std::io::prelude::*;
use std::{env, error::Error, fs, fs::File, path::Path};

// Specify the correct GLSL version in the shaders at build time.
fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let target = env::var("TARGET").unwrap();
    let shaders_files = fs::read_dir("shaders")?;

    let version = match target.as_str() {
        "wasm32-unknown-unknown" => "#version 300 es\n",
        _ => "#version 330\n",
    };

    for shader_file in shaders_files {
        if let Ok(entry) = shader_file {
            let path = entry.path();
            let mut shader_source = File::open(&path)?;

            let mut version_shader_source = String::new() + version;
            shader_source.read_to_string(&mut version_shader_source)?;

            let dest_path = Path::new(&out_dir).join(&path);
            fs::create_dir_all(Path::new(&out_dir).join(Path::new("shaders")))?;
            fs::write(&dest_path, version_shader_source.as_bytes())?;
        }
    }

    Ok(())
}
