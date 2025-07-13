struct PushConstants {
  padding: vec3<u32>,
  timestep: f32,
}

// Can't use actual push constants in wasm32.
@group(0) @binding(0) var<uniform> push_constants: PushConstants;
@group(0) @binding(1) var noise_texture: texture_2d<f32>;
@group(0) @binding(2) var linear_sampler: sampler;

@group(1) @binding(0) var velocity_texture: texture_2d<f32>;
@group(1) @binding(1) var out_velocity_texture: texture_storage_2d<rg32float, write>;

@compute
@workgroup_size(16, 16, 1)
fn main(
  @builtin(global_invocation_id) global_id: vec3<u32>,
) {
  let size = vec2<f32>(textureDimensions(out_velocity_texture));
  let sample_position = vec2<f32>(global_id.xy) / size;

  let velocity = textureLoad(velocity_texture, global_id.xy, 0).xy;
  let noise = textureSampleLevel(noise_texture, linear_sampler, sample_position, 0.0).xy;

  let newVelocity = velocity + push_constants.timestep * noise;
  textureStore(out_velocity_texture, global_id.xy, vec4<f32>(newVelocity, 0.0, 0.0));
}
