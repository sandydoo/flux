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

@group(1) @binding(0) var forward_advected_texture: texture_2d<f32>;
@group(1) @binding(1) var reverse_advected_texture: texture_2d<f32>;

@group(2) @binding(0) var velocity_texture: texture_2d<f32>;
@group(2) @binding(1) var out_velocity_texture: texture_storage_2d<rg32float, write>;

@compute
@workgroup_size(16, 16, 1)
fn main(
  @builtin(global_invocation_id) global_id: vec3<u32>,
) {
  let velocity = textureLoad(velocity_texture, global_id.xy, 0).xy;

  let size = vec2<f32>(textureDimensions(velocity_texture));
  let advected_position  = (vec2<f32>(global_id.xy) + 1.0) - uniforms.timestep * velocity;
  let min_max_sampling_position = (0.5 + floor(advected_position)) / size;
  let l = textureSampleLevel(velocity_texture, linear_sampler, min_max_sampling_position, 0.0, vec2<i32>(-1, 0)).xy;
  let r = textureSampleLevel(velocity_texture, linear_sampler, min_max_sampling_position, 0.0, vec2<i32>(1, 0)).xy;
  let b = textureSampleLevel(velocity_texture, linear_sampler, min_max_sampling_position, 0.0, vec2<i32>(0, -1)).xy;
  let t = textureSampleLevel(velocity_texture, linear_sampler, min_max_sampling_position, 0.0, vec2<i32>(0, 1)).xy;

  let min_velocity = min(l, min(r, min(t, b)));
  let max_velocity = max(l, max(r, max(t, b)));

  let forward = textureLoad(forward_advected_texture, global_id.xy, 0).xy;
  let reverse = textureLoad(reverse_advected_texture, global_id.xy, 0).xy;

  let adjusted_velocity = forward + 0.5 * (velocity - reverse);
  let new_velocity = clamp(adjusted_velocity, min_velocity, max_velocity);
  textureStore(out_velocity_texture, global_id.xy, vec4<f32>(new_velocity, 0.0, 0.0));
}
