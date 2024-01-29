// include fluid.inc
struct FluidUniforms {
  timestep: f32,
  dissipation: f32,
  alpha: f32,
  r_beta: f32,
  center_factor: f32,
  stencil_factor: f32,
  texel_size: vec2<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: FluidUniforms;
@group(0) @binding(1) var linear_sampler: sampler;

@group(1) @binding(0) var velocity_texture: texture_2d<f32>;
@group(1) @binding(1) var out_texture: texture_storage_2d<rg32float, write>;

@compute
@workgroup_size(8, 8, 1)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let texel_position = vec2<i32>(global_id.xy);
    let velocity = textureLoad(velocity_texture, texel_position, 0).xy;
    let l = textureLoad(velocity_texture, texel_position + vec2<i32>(-1, 0), 0).xy;
    let r = textureLoad(velocity_texture, texel_position + vec2<i32>(1, 0), 0).xy;
    let t = textureLoad(velocity_texture, texel_position + vec2<i32>(0, 1), 0).xy;
    let b = textureLoad(velocity_texture, texel_position + vec2<i32>(0, -1), 0).xy;

    textureStore(out_texture, texel_position, vec4<f32>(uniforms.stencil_factor * (l + r + b + t + uniforms.center_factor * velocity), 0.0, 0.0));
}
