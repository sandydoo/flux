precision mediump float;
precision mediump sampler2D;

layout(std140) uniform FluidUniforms
{
  highp float deltaT;
  highp float epsilon;
  highp float halfEpsilon;
  highp float dissipation;
  highp vec2 uTexelSize;
  lowp float pad1;
  lowp float pad2;
};

uniform sampler2D velocityTexture;

in highp vec2 textureCoord;
in vec2 vL;
in vec2 vR;
in vec2 vT;
in vec2 vB;
out vec2 newDivergence;

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
  newDivergence = vec2(div, 0.0);
}
