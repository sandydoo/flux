@group(0) @binding(0) var nearest_sampler: sampler;
@group(0) @binding(1) var out_divergence_texture: texture_storage_2d<r32float, write>;

@group(1) @binding(0) var velocity_texture: texture_2d<f32>;
@group(1) @binding(1) var out_velocity_texture: texture_storage_2d<rg32float, write>;

@compute
@workgroup_size(16, 16, 1)
fn main(
  @builtin(global_invocation_id) global_id: vec3<u32>,
) {
  let size = textureDimensions(velocity_texture);
  let sample_position = vec2<f32>(global_id.xy) / vec2<f32>(size);

  let l = textureSampleLevel(velocity_texture, nearest_sampler, sample_position, 0.0, vec2<i32>(-1, 0)).x;
  let r = textureSampleLevel(velocity_texture, nearest_sampler, sample_position, 0.0, vec2<i32>(1, 0)).x;
  let t = textureSampleLevel(velocity_texture, nearest_sampler, sample_position, 0.0, vec2<i32>(0, 1)).y;
  let b = textureSampleLevel(velocity_texture, nearest_sampler, sample_position, 0.0, vec2<i32>(0, -1)).y;

  let new_divergence = 0.5 * ((r - l) + (t - b));

  textureStore(out_divergence_texture, global_id.xy, vec4<f32>(new_divergence, 0.0, 0.0, 0.0));
}
