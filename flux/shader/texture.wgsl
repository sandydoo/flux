struct Vertex {
  position: vec2<f32>
};

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) frag_uv: vec2<f32>,
}

@group(0) @binding(0) var<storage, read> pos: array<Vertex>;
@group(0) @binding(2) var<uniform> view: vec4<f32>; // offset.xy, scale.zw

@vertex
fn vs(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
  var vertex: VertexOutput;
  let xy = pos[vertex_index].position;

  vertex.position = vec4<f32>(xy * view.zw + view.xy, 0.0, 1.0);
  vertex.frag_uv = 0.5 + 0.5 * xy;
  return vertex;
}

@group(0) @binding(1) var texture_sampler: sampler;
@group(1) @binding(0) var texture: texture_2d<f32>;

@fragment
fn fs(fs_input: VertexOutput) -> @location(0) vec4<f32> {
  // let color = textureSample(texture, texture_sampler, fs_input.frag_uv);
  // return vec4<f32>(color.r, color.g, 0.0, 1.0);
  let contrast_factor = 2.0;
  let color = 0.5 + 0.5 * textureSample(texture, texture_sampler, fs_input.frag_uv).rgb;
  return vec4<f32>(saturate(contrast_factor * (color - 0.5) + 0.5), 1.0);
}

@fragment
fn fs_scalar(fs_input: VertexOutput) -> @location(0) vec4<f32> {
  let value = textureSample(texture, texture_sampler, fs_input.frag_uv).r;
  // Boost small signed values, with a soft rolloff to preserve larger changes.
  let strength = sqrt(abs(value) * 100.0);
  let intensity = strength / (1.0 + strength);
  let negative = vec3<f32>(0.05, 0.25, 1.0);
  let positive = vec3<f32>(1.0, 0.15, 0.05);
  let color = select(negative, positive, value >= 0.0);
  return vec4<f32>(mix(vec3<f32>(0.5), color, intensity), 1.0);
}
