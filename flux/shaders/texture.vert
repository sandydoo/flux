in vec2 position;
out vec2 texturePosition;

layout(std140) uniform Projection
{
  mat4 uFluidProjection;
  mat4 uProjection;
  mat4 uView;
};

uniform float uGridWidth;
uniform float uGridHeight;

void main() {
  vec4 newPosition = uProjection * uView * vec4(vec2(uGridWidth / 2.0, uGridHeight / 2.0) * position, 0.0, 1.0);
  gl_Position = newPosition;
  texturePosition = position * 0.5 + 0.5;
}
