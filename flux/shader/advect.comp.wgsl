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

@group(1) @binding(0) var out_texture: texture_storage_2d<rg32float, write>;

@group(2) @binding(0) var<uniform> direction: Dir;

@group(3) @binding(0) var velocity_texture: texture_2d<f32>;

struct Dir {
  padding: vec3<u32>,
  direction: f32,
}

@compute
@workgroup_size(16, 16, 1)
fn main(
  @builtin(global_invocation_id) global_id: vec3<u32>,
) {
  let velocity = textureLoad(velocity_texture, global_id.xy, 0).xy;

  // Note, that, by multiplying by dx, we’ve “incorrectly” scaled our coordinate system.
  // This is actually a key component of the slow, wriggly “coral reef” look.
  let size = vec2<f32>(textureDimensions(velocity_texture));
  let sample_position = vec2<f32>(global_id.xy);

  let advected_position = ((sample_position + 0.5) + direction.direction * uniforms.timestep * velocity) / size;
  let decay = 1.0 + uniforms.dissipation * uniforms.timestep;
  let new_velocity = textureSampleLevel(velocity_texture, linear_sampler, advected_position, 0.0).xy / decay;
  textureStore(out_texture, global_id.xy, vec4<f32>(new_velocity, 0.0, 0.0));
}
