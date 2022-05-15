precision mediump float;
precision highp sampler2D;

layout(std140) uniform FluidUniforms
{
  highp float deltaT;
  highp float dissipation;
  highp vec2 uTexelSize;
};

uniform sampler2D velocityTexture;

in highp vec2 texturePosition;
in vec2 vL;
in vec2 vR;
in vec2 vT;
in vec2 vB;
out float newDivergence;

void main() {
  float L = texture(velocityTexture, vL).x;
  float R = texture(velocityTexture, vR).x;
  float T = texture(velocityTexture, vT).y;
  float B = texture(velocityTexture, vB).y;

  vec2 velocity = texture(velocityTexture, texturePosition).xy;
  if (vL.x < 0.0) { L = -velocity.x; }
  if (vR.x > 1.0) { R = -velocity.x; }
  if (vT.y > 1.0) { T = -velocity.y; }
  if (vB.y < 0.0) { B = -velocity.y; }

  newDivergence = 0.5 * (R - L + T - B);
}
