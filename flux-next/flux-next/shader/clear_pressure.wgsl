@group(0) @binding(0) var input_texture: texture_storage_2d<r32float, write>;

@compute
@workgroup_size(8, 8, 1)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) num_workgroups: vec3<u32>,
) {
    let position = vec2<i32>(global_id.xy);
    textureStore(input_texture, position, vec4(0.0, 0.0, 0.0, 0.0));
}
