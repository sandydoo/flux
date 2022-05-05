precision mediump float;
precision highp sampler2D;

layout(std140) uniform FluidUniforms
{
  highp float deltaT;
  highp float epsilon;
  highp float halfEpsilon;
  highp float dissipation;
  highp vec2 uTexelSize;
};

uniform sampler2D velocityTexture;
uniform float amount;

in vec2 textureCoord;
out vec2 outVelocity;

void main() {
  vec2 velocity = texture(velocityTexture, textureCoord).xy;
  vec2 advectedCoord = textureCoord - amount * velocity;
  float decay = 1.0 + dissipation * amount;
  outVelocity = texture(velocityTexture, advectedCoord).xy / decay;
}
