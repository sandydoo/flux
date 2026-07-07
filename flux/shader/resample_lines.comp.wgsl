// One-shot pass run when the grid dimensions change. Every grid spans the same
// normalized [0,1]² square (basepoint = index / (dim - 1)), so the old and new
// grids are two samplings of the same continuous field. Each new line inherits
// the state of the nearest old line at the same normalized position.
//
// Nearest-neighbour (rather than bilinear) is deliberate: window drags make the
// grid dimensions oscillate by ±1 at every column boundary, and a round trip of
// nearest-neighbour resamples is exactly the identity — a row duplicated on grow
// is dropped again on shrink, with no state change. Bilinear would low-pass the
// line state a little on every toggle, which the springs then fight, producing a
// visible up/down jitter as the window resizes.
//
// Thanks to scale-invariant line state (plan 1) the inherited state is already
// valid in the new grid — no scale correction is needed. Basepoints are not
// resampled: the resampled positions equal the new grid's basepoints exactly, so
// the new (target) grid positions are used directly.

struct Line {
  endpoint: vec2<f32>,
  velocity: vec2<f32>,
  color: vec4<f32>,
  color_velocity: vec3<f32>,
  width: f32,
}

struct ResampleUniforms {
  old_columns: u32,
  old_rows: u32,
  new_columns: u32,
  new_rows: u32,
}

@group(0) @binding(0) var<uniform> params: ResampleUniforms;
@group(0) @binding(1) var<storage, read> old_lines: array<Line>;
@group(0) @binding(2) var<storage, read_write> new_lines: array<Line>;

@compute
@workgroup_size(64)
fn main(
  @builtin(global_invocation_id) global_id: vec3<u32>,
) {
  let index = global_id.x;
  if (index >= arrayLength(&new_lines)) {
    return;
  }

  let u = index % params.new_columns;
  let v = index / params.new_columns;

  // Map the new grid position to the nearest old grid node. Both grids place
  // node i at i / (dim - 1), so old_index = new_index * (old_dim - 1) / (new_dim - 1).
  let fx = f32(u) * f32(params.old_columns - 1u) / f32(max(params.new_columns - 1u, 1u));
  let fy = f32(v) * f32(params.old_rows - 1u) / f32(max(params.new_rows - 1u, 1u));

  let ox = min(u32(round(fx)), params.old_columns - 1u);
  let oy = min(u32(round(fy)), params.old_rows - 1u);

  new_lines[index] = old_lines[oy * params.old_columns + ox];
}
