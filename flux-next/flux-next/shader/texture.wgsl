struct Vertex {
  position: vec2<f32>
};

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) texture_position: vec2<f32>,
}

@group(0) @binding(0) var<storage, read> pos: array<Vertex>;

@vertex
fn vs(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var vertex: VertexOutput;
    let xy = pos[vertex_index].position;

    vertex.position = vec4<f32>(xy, 0.0, 1.0);
    vertex.texture_position = xy;
    return vertex;
}

@group(0) @binding(1) var texture: texture_2d<f32>;
@group(0) @binding(2) var texture_sampler: sampler;

@fragment
fn fs(fs_input: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(texture, texture_sampler, fs_input.texture_position);
    return vec4<f32>(color.r, color.g, 0.0, 1.0);
}
