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
uniform sampler2D pressureTexture;

in vec2 textureCoord;
in vec2 vL;
in vec2 vR;
in vec2 vT;
in vec2 vB;
out vec2 newVelocity;

void main() {
  vec2 velocity = texture(velocityTexture, textureCoord).xy;

  float L = texture(pressureTexture, vL).x;
  float R = texture(pressureTexture, vR).x;
  float T = texture(pressureTexture, vT).x;
  float B = texture(pressureTexture, vB).x;

  newVelocity = velocity - halfEpsilon * vec2(R - L, T - B);
}
