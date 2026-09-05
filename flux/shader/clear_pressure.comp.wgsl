struct FluidUniforms {
  timestep: f32,
  dissipation: f32,
  inverse_cell: vec2<f32>,
  velocity_to_uv: vec2<f32>,
  diffusion_weight: vec2<f32>,
  pressure_clear: f32,
}

@group(0) @binding(0) var<uniform> uniforms: FluidUniforms;
@group(1) @binding(1) var out_pressure_texture: texture_storage_2d<r32float, write>;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
  textureStore(out_pressure_texture, id.xy, vec4<f32>(uniforms.pressure_clear, 0.0, 0.0, 0.0));
}
