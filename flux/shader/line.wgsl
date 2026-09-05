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
}

@group(0) @binding(0) var<uniform> uniforms: LineUniforms;
@group(1) @binding(0) var<uniform> view_matrix: mat4x4<f32>;

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) f_vertex: vec2<f32>,
  @location(1) f_color: vec4<f32>,
}

@vertex
fn main_vs(
  @location(0) endpoint: vec2<f32>, // 0
  @location(1) velocity: vec2<f32>, // 8
  @location(2) color: vec4<f32>, // 16
  @location(3) color_velocity: vec3<f32>, // 32
  @location(4) width: f32, // 44
  @location(5) basepoint: vec2<f32>, // 48
  @location(6) vertex: vec2<f32>, // 56
) -> VertexOutput { // 64
  var x_basis = vec2<f32>(-endpoint.y, endpoint.x);
  x_basis /= max(length(x_basis), 1e-10); // safely normalize

  let line_position = mix(uniforms.line_begin_offset, 1.0, vertex.y);
  var point = vec2<f32>(uniforms.aspect, 1.0) * uniforms.zoom * (basepoint * 2.0 - 1.0)
    + uniforms.line_length * endpoint * line_position
    + uniforms.line_width * width * x_basis * vertex.x;

  point.x /= uniforms.aspect;

  let radius = 0.5 * uniforms.line_width * width;
  let short_line_boost = 1.0 + radius / max(length(uniforms.line_length * endpoint), 1e-10);
  // Evaluate Drift's fade at the vertices. Rasterization interpolates alpha
  // linearly along the body; smoothstep in the fragment changes that profile.
  let fade = smoothstep(0.0, 1.0, vertex.y * line_position / short_line_boost);

  let transformed_point = view_matrix * vec4<f32>(point, 0.0, 1.0);

  return VertexOutput(
    transformed_point,
    vertex,
    vec4<f32>(color.rgb, color.a * fade),
  );
}

@fragment
fn main_fs(fs_input: VertexOutput) -> @location(0) vec4<f32> {
  let edge_width = fwidth(fs_input.f_vertex.x);
  let x_offset = abs(fs_input.f_vertex.x);
  let smooth_edges = 1.0 - smoothstep(0.5 - edge_width, 0.5, x_offset);

  return vec4<f32>(fs_input.f_color.rgb, fs_input.f_color.a * smooth_edges);
}
