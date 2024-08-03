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

@group(1) @binding(0) var pressure_texture: texture_2d<f32>;

@group(2) @binding(0) var velocity_texture: texture_2d<f32>;
@group(2) @binding(1) var out_velocity_texture: texture_storage_2d<rg32float, write>;

@compute
@workgroup_size(16, 16, 1)
fn main(
  @builtin(global_invocation_id) global_id: vec3<u32>,
) {
  let size = textureDimensions(velocity_texture);
  let sample_position = vec2<f32>(global_id.xy) / vec2<f32>(size);

  let pressure = textureLoad(pressure_texture, global_id.xy, 0).x;

  var l = textureSampleLevel(pressure_texture, linear_sampler, sample_position, 0.0, vec2<i32>(-1, 0)).x;
  var r = textureSampleLevel(pressure_texture, linear_sampler, sample_position, 0.0, vec2<i32>(1, 0)).x;
  var b = textureSampleLevel(pressure_texture, linear_sampler, sample_position, 0.0, vec2<i32>(0, -1)).x;
  var t = textureSampleLevel(pressure_texture, linear_sampler, sample_position, 0.0, vec2<i32>(0, 1)).x;

  // Enforce the following boundary conditions:
  //
  //  1. No-slip condition — velocity equals zero at the boundaries.
  //
  //  2. Pure Neumann pressure condition — dp/dn = 0, that is the rate of change
  //     of pressure in the direction normal to the boundary is zero.
  //
  //  GPU Gems has a short section deriving these conditions, but this
  //  implementation is slightly different.
  //
  //  Here, we’re assuming the boundary is the outer edge of the texture grid.
  //
  //  For condition 1, we just set the velocity to zero.
  //
  //  For condition 2, we don’t have to do anything. With texture clamping, any
  //  pressure reads outside the boundary will be set to the last value at the
  //  boundary; so the rate of change across the boundary becomes zero.
  //
  //  I haven’t tested this with an ink/particle texture, so there’s a chance
  //  this doesn’t actually look any good. But it is stable! I’m also unsure of
  //  how the staggered grid affects all of this.
  //
  //  A number of things actually work here: -1.0 adjustment for velocity,
  //  setting just the relevant component of velocity to zero, and flipping
  //  pressures along relevant axis. All seem stable, but experiment!

  var boundary_condition = vec2<f32>(1.0);
  if (global_id.x == 0u) {
    boundary_condition.x = 0.0;
  } else if (global_id.x == size.x - 1u) {
    boundary_condition.x = 0.0;
  }
  if (global_id.y == 0u) {
    boundary_condition.y = 0.0;
  } else if (global_id.y == size.y - 1u) {
    boundary_condition.y = 0.0;
  }

  let velocity = textureLoad(velocity_texture, global_id.xy, 0).xy;
  let new_velocity = boundary_condition * (velocity - 0.5 * vec2<f32>(r - l, t - b));

  textureStore(out_velocity_texture, global_id.xy, vec4<f32>(new_velocity, 0.0, 0.0));
}
