// Carry fixed-unit velocities through the centered world-space overlap.
// Newly revealed world space starts at rest instead of stretching edge flow.
struct DomainChange { ratio: vec2<f32>, padding: vec2<f32> }
@group(0) @binding(3) var<uniform> domain: DomainChange;

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

  let uv = (vec2<f32>(global_id.xy) + 0.5) / vec2<f32>(out_size);
  let sample_position = (uv - 0.5) * domain.ratio + 0.5;
  var velocity = vec2<f32>(0.0);
  if (all(sample_position >= vec2<f32>(0.0)) && all(sample_position <= vec2<f32>(1.0))) {
    velocity = textureSampleLevel(velocity_texture, linear_sampler, sample_position, 0.0).xy;
  }

  textureStore(out_velocity_texture, global_id.xy, vec4<f32>(velocity, 0.0, 0.0));
}
