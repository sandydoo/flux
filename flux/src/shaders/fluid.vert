#version 300 es
precision highp float;

in vec3 position;

layout(std140) uniform Uniforms
{
  highp float deltaT;
  highp float pad1;
  highp vec2 uTexelSize;
  highp float epsilon;
  highp float halfEpsilon;
  highp float dissipation;
  highp float padding;
};

out vec2 textureCoord;
out vec2 vL;
out vec2 vR;
out vec2 vT;
out vec2 vB;

void main() {
  gl_Position = vec4(position, 1.0);
  textureCoord = position.xy * 0.5 + 0.5;

  vL = textureCoord - vec2(uTexelSize.x, 0.0);
  vR = textureCoord + vec2(uTexelSize.x, 0.0);
  vT = textureCoord + vec2(0.0, uTexelSize.y);
  vB = textureCoord - vec2(0.0, uTexelSize.y);
}
