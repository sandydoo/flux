#version 300 es
precision mediump float;
precision mediump sampler2D;

uniform float halfEpsilon;
uniform sampler2D velocityTexture;

in highp vec2 textureCoord;
in vec2 vL;
in vec2 vR;
in vec2 vT;
in vec2 vB;
out vec4 fragColor;

void main() {
  float L = texture(velocityTexture, vL).x;
  float R = texture(velocityTexture, vR).x;
  float T = texture(velocityTexture, vT).y;
  float B = texture(velocityTexture, vB).y;

  vec2 velocity = texture(velocityTexture, textureCoord).xy;
  if (vL.x < 0.0) { L = -velocity.x; }
  if (vR.x > 1.0) { R = -velocity.x; }
  if (vT.y > 1.0) { T = -velocity.y; }
  if (vB.y < 0.0) { B = -velocity.y; }

  float div = halfEpsilon * (R - L + T - B);
  fragColor = vec4(div, 0.0, 0.0, 1.0);
}
