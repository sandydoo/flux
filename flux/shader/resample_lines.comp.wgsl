// One-shot pass run when the grid dimensions change (window resize or a
// grid_spacing change). Each line is identified by its integer offset from the
// grid centre. A line at centre-offset (du, dv) inherits the state of the line
// at the same offset in the old grid, so every line that exists in both grids
// keeps its identity — and therefore its on-screen position, since the view is
// zoomed in and lines sit on a fixed centred lattice. Lines with no counterpart
// (the newly-exposed edges) are born empty and fade in via the velocity-driven
// width, and lines that fall off the edge are simply dropped.
//
// This is why the counts are forced odd (see `grid.rs`): an odd count puts a
// line exactly at the centre, so the offsets are integers in both grids and the
// mapping is exact — no half-cell shimmer as the counts change.
//
// The pass also seeds each line's *current* basepoint, which then eases toward
// its target in place_lines:
//   - snap != 0 (window resize): seed at the target. A surviving line's target
//     already equals its old on-screen position, so nothing moves; only the
//     edges change.
//   - snap == 0 (grid_spacing change): seed at the line's old position so it
//     glides from the old spacing to the new one — visible positional movement.

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
  snap: u32,
}

@group(0) @binding(0) var<uniform> params: ResampleUniforms;
@group(0) @binding(1) var<storage, read> old_lines: array<Line>;
@group(0) @binding(2) var<storage, read_write> new_lines: array<Line>;
@group(0) @binding(3) var<storage, read> old_basepoints: array<vec2<f32>>;
@group(0) @binding(4) var<storage, read> target_basepoints: array<vec2<f32>>;
@group(0) @binding(5) var<storage, read_write> new_basepoints: array<vec2<f32>>;

@compute
@workgroup_size(64)
fn main(
  @builtin(global_invocation_id) global_id: vec3<u32>,
) {
  let index = global_id.x;
  if (index >= arrayLength(&new_lines)) {
    return;
  }

  let u = i32(index % params.new_columns);
  let v = i32(index / params.new_columns);

  // Same centre-offset in the old grid. Counts are odd, so (count - 1) / 2 is
  // the exact integer centre index in both grids.
  let u_old = u - (i32(params.new_columns) - 1) / 2 + (i32(params.old_columns) - 1) / 2;
  let v_old = v - (i32(params.new_rows) - 1) / 2 + (i32(params.old_rows) - 1) / 2;

  let old_valid = u_old >= 0 && u_old < i32(params.old_columns)
    && v_old >= 0 && v_old < i32(params.old_rows);

  if (old_valid) {
    let old_index = u32(v_old) * params.old_columns + u32(u_old);
    new_lines[index] = old_lines[old_index];
    if (params.snap != 0u) {
      new_basepoints[index] = target_basepoints[index];
    } else {
      new_basepoints[index] = old_basepoints[old_index];
    }
  } else {
    // Newly-exposed edge line: born empty at its target, fades in on its own.
    new_lines[index] = Line(vec2<f32>(0.0), vec2<f32>(0.0), vec4<f32>(0.0), vec3<f32>(0.0), 0.0);
    new_basepoints[index] = target_basepoints[index];
  }
}
