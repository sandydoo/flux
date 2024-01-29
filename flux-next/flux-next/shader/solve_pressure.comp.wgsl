// include fluid.inc
struct FluidUniforms {
  timestep: f32,
  dissipation: f32,
  alpha: f32,
  r_beta: f32,
  center_factor: f32,
  stencil_factor: f32,
  texel_size: vec2<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: FluidUniforms;
@group(0) @binding(1) var linear_sampler: sampler;

@group(1) @binding(0) var divergence_texture: texture_2d<f32>;

@group(2) @binding(0) var pressure_texture: texture_2d<f32>;
@group(2) @binding(1) var out_pressure_texture: texture_storage_2d<r32float, write>;

@compute
@workgroup_size(8, 8, 1)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
  let size = vec2<f32>(textureDimensions(pressure_texture));
  let texel_position = vec2<i32>(global_id.xy);
  let sample_position = vec2<f32>(global_id.xy) / size;

  let pressure = textureLoad(pressure_texture, texel_position, 0).x;
  let divergence = textureLoad(divergence_texture, texel_position, 0).x;

  let l = textureSampleLevel(pressure_texture, linear_sampler, sample_position, 0.0, vec2<i32>(-1, 0)).x;
  let r = textureSampleLevel(pressure_texture, linear_sampler, sample_position, 0.0, vec2<i32>(1, 0)).x;
  let t = textureSampleLevel(pressure_texture, linear_sampler, sample_position, 0.0, vec2<i32>(0, 1)).x;
  let b = textureSampleLevel(pressure_texture, linear_sampler, sample_position, 0.0, vec2<i32>(0, -1)).x;

  let newPressure = uniforms.r_beta * (l + r + b + t + uniforms.alpha * divergence);

  textureStore(out_pressure_texture, texel_position, vec4<f32>(newPressure, 0.0, 0.0, 0.0));
}