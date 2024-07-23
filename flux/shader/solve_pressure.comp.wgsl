// include fluid.inc
struct FluidUniforms {
  timestep: f32,
  dissipation: f32,
  alpha: f32,
  r_beta: f32,
  center_factor: f32,
  stencil_factor: f32,
}

@group(0) @binding(0) var<uniform> uniforms: FluidUniforms;
@group(0) @binding(1) var linear_sampler: sampler;
@group(0) @binding(2) var nearest_sampler: sampler;

@group(1) @binding(0) var divergence_texture: texture_2d<f32>;

@group(2) @binding(0) var pressure_texture: texture_2d<f32>;
@group(2) @binding(1) var out_pressure_texture: texture_storage_2d<r32float, write>;

@compute
@workgroup_size(16, 16, 1)
fn main(
  @builtin(global_invocation_id) global_id: vec3<u32>,
) {
  let size = textureDimensions(pressure_texture);
  let sample_position = vec2<f32>(global_id.xy) / vec2<f32>(size);

  let pressure = textureLoad(pressure_texture, global_id.xy, 0).x;
  let divergence = textureLoad(divergence_texture, global_id.xy, 0).x;

  var l = textureSampleLevel(pressure_texture, nearest_sampler, sample_position, 0.0, vec2<i32>(-1, 0)).x;
  var r = textureSampleLevel(pressure_texture, nearest_sampler, sample_position, 0.0, vec2<i32>(1, 0)).x;
  var b = textureSampleLevel(pressure_texture, nearest_sampler, sample_position, 0.0, vec2<i32>(0, -1)).x;
  var t = textureSampleLevel(pressure_texture, nearest_sampler, sample_position, 0.0, vec2<i32>(0, 1)).x;

  if (global_id.x == 0u) {
    l = pressure;
  } else if (global_id.x == size.x - 1u) {
    r = pressure;
  }
  if (global_id.y == 0u) {
    b = pressure;
  } else if (global_id.y == size.y - 1u) {
    t = pressure;
  }

  let new_pressure = uniforms.r_beta * (l + r + b + t + uniforms.alpha * divergence);

  textureStore(out_pressure_texture, global_id.xy, vec4<f32>(new_pressure, 0.0, 0.0, 0.0));
}
