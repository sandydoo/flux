#ifdef GL_ES
precision highp float;
#endif

layout(std140) uniform FluidUniforms
{
  highp float deltaT;
  highp float dissipation;
  highp vec2 uTexelSize;
};

in vec3 position;

out vec2 texturePosition;
out vec2 vL;
out vec2 vR;
out vec2 vT;
out vec2 vB;

void main() {
  gl_Position = vec4(position, 1.0);
  texturePosition = position.xy * 0.5 + 0.5;

  vL = texturePosition + vec2(-uTexelSize.x, 0.0);
  vR = texturePosition + vec2(uTexelSize.x, 0.0);
  vT = texturePosition + vec2(0.0, uTexelSize.y);
  vB = texturePosition + vec2(0.0, -uTexelSize.y);
}
