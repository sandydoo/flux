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

@group(1) @binding(0) var velocity_texture: texture_2d<f32>;
@group(1) @binding(1) var out_texture: texture_storage_2d<rg32float, write>;

@compute
@workgroup_size(16, 16, 1)
fn main(
  @builtin(global_invocation_id) global_id: vec3<u32>,
) {
  let velocity = textureLoad(velocity_texture, global_id.xy, 0).xy;

  let size = textureDimensions(velocity_texture, 0);
  let sample_position = vec2<f32>(global_id.xy) / vec2<f32>(size);
  let l = textureSampleLevel(velocity_texture, nearest_sampler, sample_position, 0.0, vec2<i32>(-1, 0)).xy;
  let r = textureSampleLevel(velocity_texture, nearest_sampler, sample_position, 0.0, vec2<i32>(1, 0)).xy;
  let b = textureSampleLevel(velocity_texture, nearest_sampler, sample_position, 0.0, vec2<i32>(0, -1)).xy;
  let t = textureSampleLevel(velocity_texture, nearest_sampler, sample_position, 0.0, vec2<i32>(0, 1)).xy;

  let new_velocity = uniforms.stencil_factor * (l + r + b + t + uniforms.center_factor * velocity);

  textureStore(out_texture, global_id.xy, vec4<f32>(new_velocity, 0.0, 0.0));
}
