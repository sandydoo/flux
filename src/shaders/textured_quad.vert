#version 300 es
precision highp float;

in vec3 position;

uniform float uTexelSize;

out vec2 textureCoord;
out vec2 vL;
out vec2 vR;
out vec2 vT;
out vec2 vB;

void main() {
  gl_Position = vec4(position, 1.0);
  textureCoord = position.xy * 0.5 + 0.5;

  vL = textureCoord - vec2(uTexelSize, 0.0);
  vR = textureCoord + vec2(uTexelSize, 0.0);
  vT = textureCoord + vec2(0.0, uTexelSize);
  vB = textureCoord - vec2(0.0, uTexelSize);
}
