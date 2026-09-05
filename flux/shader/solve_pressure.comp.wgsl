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

@group(1) @binding(0) var divergence_texture: texture_2d<f32>;

@group(2) @binding(0) var pressure_texture: texture_2d<f32>;
@group(2) @binding(1) var out_pressure_texture: texture_storage_2d<r32float, write>;

@compute
@workgroup_size(16, 16, 1)
fn main(
  @builtin(global_invocation_id) global_id: vec3<u32>,
) {
  let size = textureDimensions(pressure_texture);
  let position = vec2<i32>(global_id.xy);
  let last = vec2<i32>(size) - 1;
  let divergence = textureLoad(divergence_texture, global_id.xy, 0).x;

  let l = textureLoad(pressure_texture, clamp(position + vec2<i32>(-1, 0), vec2<i32>(0), last), 0).x;
  let r = textureLoad(pressure_texture, clamp(position + vec2<i32>(1, 0), vec2<i32>(0), last), 0).x;
  let b = textureLoad(pressure_texture, clamp(position + vec2<i32>(0, -1), vec2<i32>(0), last), 0).x;
  let t = textureLoad(pressure_texture, clamp(position + vec2<i32>(0, 1), vec2<i32>(0), last), 0).x;

  let weight = uniforms.inverse_cell * uniforms.inverse_cell;
  let new_pressure = (weight.x * (l + r) + weight.y * (b + t) - divergence)
      / (2.0 * (weight.x + weight.y));

  textureStore(out_pressure_texture, global_id.xy, vec4<f32>(new_pressure, 0.0, 0.0, 0.0));
}
