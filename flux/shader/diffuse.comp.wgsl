// include fluid.inc
struct FluidUniforms {
  timestep: f32,
  dissipation: f32,
  inverse_cell: vec2<f32>,
  velocity_to_uv: vec2<f32>,
  diffusion_weight: vec2<f32>,
  pressure_clear: f32,
}

@group(0) @binding(0) var<uniform> uniforms: FluidUniforms;
@group(0) @binding(1) var linear_sampler: sampler;
@group(0) @binding(2) var nearest_sampler: sampler;

@group(1) @binding(0) var velocity_texture: texture_2d<f32>;
@group(1) @binding(1) var out_texture: texture_storage_2d<rgba16float, write>;

@compute
@workgroup_size(16, 16, 1)
fn main(
  @builtin(global_invocation_id) global_id: vec3<u32>,
) {
  let velocity = textureLoad(velocity_texture, global_id.xy, 0).xy;

  let size = textureDimensions(velocity_texture, 0);
  // Start at texel centres before applying the world-space stencil.
  let sample_position = (vec2<f32>(global_id.xy) + 0.5) / vec2<f32>(size);
  // Fixed world distances preserve the tuned 128×128/16:10 stencil. At
  // doubled quality these taps are two texels away, with identical strength.
  let dx = vec2<f32>(uniforms.velocity_to_uv.x, 0.0);
  let dy = vec2<f32>(0.0, uniforms.velocity_to_uv.y);
  let l = textureSampleLevel(velocity_texture, linear_sampler, sample_position - dx, 0.0).xy;
  let r = textureSampleLevel(velocity_texture, linear_sampler, sample_position + dx, 0.0).xy;
  let b = textureSampleLevel(velocity_texture, linear_sampler, sample_position - dy, 0.0).xy;
  let t = textureSampleLevel(velocity_texture, linear_sampler, sample_position + dy, 0.0).xy;

  let new_velocity = velocity + uniforms.diffusion_weight.x * (l + r - 2.0 * velocity)
      + uniforms.diffusion_weight.y * (b + t - 2.0 * velocity);

  textureStore(out_texture, global_id.xy, vec4<f32>(new_velocity, 0.0, 0.0));
}
