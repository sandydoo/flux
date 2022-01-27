in vec3 position;
out vec2 textureCoord;

layout(std140) uniform Projection
{
  mat4 uProjection;
  mat4 uView;
};

void main() {
  vec4 newPosition = uView * vec4(position, 1.0);
  gl_Position = newPosition;
  textureCoord = newPosition.xy * 0.5 + 0.5;
}
