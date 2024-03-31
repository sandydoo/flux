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

struct VertexOutput {
  @builtin(position) f_position: vec4<f32>,
  @location(0) f_vertex: vec2<f32>,
  @location(1) f_mindpoint_vector: vec2<f32>,
  @location(2) f_top_color: vec4<f32>,
  @location(3) f_bottom_color: vec4<f32>,
};

// TODO: you can use storage buffers for this instead of messing around with vertex buffers.
@vertex
fn main_vs(
  @builtin(vertex_index) vindex: u32,
  @location(0) endpoint: vec2<f32>, // 0
  @location(1) velocity: vec2<f32>, // 8
  @location(2) color: vec4<f32>, // 16
  @location(3) color_velocity: vec3<f32>, // 32
  @location(4) width: f32, // 44
  @location(5) _vertex: vec2<f32>, // 48
  @location(6) basepoint: vec2<f32>, // 56
) -> VertexOutput {
  // TODO: var is not a good idea. Use vertex or storage buffer.
  var vertices = array(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>(-1.0, 1.0),
    vec2<f32>(1.0, -1.0),
    vec2<f32>(1.0, -1.0),
    vec2<f32>(-1.0, 1.0),
    vec2<f32>(1.0, 1.0),
  );
  let vertex = vertices[vindex];

  var point
    = vec2<f32>(uniforms.aspect, 1.0) * uniforms.zoom * (basepoint * 2.0 - 1.0)
    + endpoint
    + 0.5 * uniforms.line_width * width * vertex;

  point.x /= uniforms.aspect;

  // Rotate the endpoint vector 90°. We use this to detect which side of the
  // endpoint we’re on in the fragment.
  let midpoint_vector = vec2<f32>(endpoint.y, -endpoint.x);

  let endpoint_threshold = 1.0;
  let endpoint_opacity = clamp(color.a + max(0.0, endpoint_threshold - color.a), 0.0, 1.0);
  let top_color = vec4<f32>(color.rgb, endpoint_opacity);

  // The color of the lower half of the endpoint is less obvious. We’re
  // drawing over part of the line, so to match the color of the upper
  // endpoint, we have to do some math. Luckily, we know the premultiplied
  // color of the line underneath, so we can reverse the blend equation to get
  // the right color.
  //
  // GL_BLEND(SRC_ALPHA, ONE) = srcColor * srcAlpha + dstColor * srcAlpha
  // = vColor * vEndpointOpacity + vColor * vLineOpacity
  //
  // Remember, we’ve already premultiplied our colors! The opacity should be
  // 1.0 to disable more opacity blending!
  let premultiplied_color = color.rgb * color.a;
  let bottom_color = vec4<f32>(color.rgb * endpoint_opacity - premultiplied_color, 1.0);

  return VertexOutput(
    vec4<f32>(point, 0.0, 1.0),
    vertex,
    midpoint_vector,
    top_color,
    bottom_color,
  );
}

@fragment
fn main_fs(fs_input: VertexOutput) -> @location(0) vec4<f32> {
  var color = fs_input.f_bottom_color;

  // Test which side of the endpoint we’re on.
  let side
    = (fs_input.f_vertex.x - fs_input.f_mindpoint_vector.x) * (-fs_input.f_mindpoint_vector.y)
    - (fs_input.f_vertex.y - fs_input.f_mindpoint_vector.y) * (-fs_input.f_mindpoint_vector.x);

  if (side > 0.0) {
    color = fs_input.f_top_color;
  }

  let distance = length(fs_input.f_vertex);
  let smoothEdges = 1.0 - smoothstep(1.0 - fwidth(distance), 1.0, distance);
  return vec4<f32>(color.rgb, color.a * smoothEdges);
}