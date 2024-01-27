@group(0) @binding(0) var noise_texture: texture_2d<f32>;
@group(0) @binding(1) var linear_sampler: sampler;

@group(1) @binding(0) var velocity_texture: texture_2d<f32>;
@group(1) @binding(1) var out_velocity_texture: texture_storage_2d<rg32float, write>;

var<push_constant> timestep: f32;

@compute
@workgroup_size(8, 8, 1)
fn main(
  @builtin(global_invocation_id) global_id: vec3<u32>,
) {
  let dims = vec2<f32>(textureDimensions(out_velocity_texture));
  let texel_position = vec2<f32>(global_id.xy) / dims;
  // let velocity = textureLoad(velocity_texture, global_id.xy).xy;
  let velocity = textureSampleLevel(velocity_texture, linear_sampler, texel_position, 0.0).xy;
  let noise = textureSampleLevel(noise_texture, linear_sampler, texel_position, 0.0).xy;
  textureStore(out_velocity_texture, global_id.xy, vec4<f32>(velocity + timestep * noise, 0.0, 0.0));
}