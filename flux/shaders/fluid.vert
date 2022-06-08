#ifdef GL_ES
precision highp float;
#endif

layout(std140) uniform FluidUniforms
{
  highp float deltaT;
  highp float dissipation;
  highp vec2 uTexelSize;
};

in vec2 position;

out vec2 texturePosition;

void main() {
  gl_Position = vec4(position, 0.0, 1.0);
  texturePosition = position * 0.5 + 0.5;
}
