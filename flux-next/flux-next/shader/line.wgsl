struct LineUniforms {
  aspect: f32,
  zoom: f32,
  line_width: f32,
  line_length: f32,
  line_begin_offset: f32,
  line_variance: f32,
  line_noise_scale: vec2<f32>,
  line_noise_offset_1: f32,
  line_noise_offset_2: f32,
  line_noise_blend_factor: f32,
  color_mode: u32,
  delta_time: f32,
  padding: f32,
}

@group(0) @binding(0) var<uniform> uniforms: LineUniforms;

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) f_vertex: vec2<f32>,
  @location(1) f_color: vec4<f32>,
  @location(2) f_line_offset: f32,
}

@vertex
fn main_vs(
  @location(0) endpoint: vec2<f32>, // 0
  @location(1) velocity: vec2<f32>, // 8
  @location(2) color: vec4<f32>, // 16
  @location(3) color_velocity: vec3<f32>, // 32
  @location(4) width: f32, // 44
  @location(5) vertex: vec2<f32>, // 48
  @location(6) basepoint: vec2<f32>, // 56
) -> VertexOutput { // 64
  var x_basis = vec2<f32>(-endpoint.y, endpoint.x);
  x_basis /= length(x_basis) + 0.0001; // safely normalize

  var point = vec2<f32>(uniforms.aspect, 1.0) * uniforms.zoom * (basepoint * 2.0 - 1.0)
    + endpoint * vertex.y
    + uniforms.line_width * width * x_basis * vertex.x;
  
  point.x /= uniforms.aspect;

  let short_line_boost = 1.0 + (uniforms.line_width * width) / length(endpoint);
  let line_offset = uniforms.line_begin_offset / short_line_boost;

  return VertexOutput(
    vec4<f32>(point, 0.0, 1.0),
    vertex,
    color,
    line_offset,
  );
}

@fragment
fn main_fs(fs_input: VertexOutput) -> @location(0) vec4<f32> {
  let fade = smoothstep(fs_input.f_line_offset, 1.0, fs_input.f_vertex.y);

  let x_offset = abs(fs_input.f_vertex.x);
  let smooth_edges = 1.0 - smoothstep(0.5 - fwidth(x_offset), 0.5, x_offset);

  return vec4<f32>(fs_input.f_color.rgb, fs_input.f_color.a * fade * smooth_edges);
}