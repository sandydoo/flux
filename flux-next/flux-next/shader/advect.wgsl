// include fluid.inc

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@compute @workgroup_size(16, 16, 1)
fn main(
  @builtin(global_invocation_id) global_id: vec3<u32>,
  @builtin(num_workgroups) num_workgroups: vec3<u32>,
) { }
