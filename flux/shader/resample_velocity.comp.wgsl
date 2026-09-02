// One-shot pass run when the fluid changes size. It carries the velocity field
// into the new texture so the flow continues instead of restarting.
//
// Velocity is measured in texels per second. A texel covers a different
// fraction of the screen in the new texture, so each component is scaled by
// the change in texel density along its axis. The flow then covers the same
// fraction of the screen per second as before.

@group(0) @binding(0) var linear_sampler: sampler;
@group(0) @binding(1) var velocity_texture: texture_2d<f32>;
@group(0) @binding(2) var out_velocity_texture: texture_storage_2d<rgba16float, write>;

@compute
@workgroup_size(16, 16, 1)
fn main(
  @builtin(global_invocation_id) global_id: vec3<u32>,
) {
  let out_size = textureDimensions(out_velocity_texture);
  if (global_id.x >= out_size.x || global_id.y >= out_size.y) {
    return;
  }

  let in_size = vec2<f32>(textureDimensions(velocity_texture));
  let sample_position = (vec2<f32>(global_id.xy) + 0.5) / vec2<f32>(out_size);
  let scale = vec2<f32>(out_size) / in_size;
  let velocity = textureSampleLevel(velocity_texture, linear_sampler, sample_position, 0.0).xy * scale;

  textureStore(out_velocity_texture, global_id.xy, vec4<f32>(velocity, 0.0, 0.0));
}
