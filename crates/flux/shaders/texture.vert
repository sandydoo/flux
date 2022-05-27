in vec2 position;
out vec2 texturePosition;

layout(std140) uniform Projection
{
  mat4 uProjection;
  mat4 uView;
};

void main() {
  vec4 newPosition = uView * vec4(position, 0.0, 1.0);
  gl_Position = newPosition;
  texturePosition = newPosition.xy * 0.5 + 0.5;
}
