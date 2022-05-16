#ifdef GL_ES
precision mediump float;
precision highp sampler2D;
#endif

layout(std140) uniform FluidUniforms
{
  highp float deltaT;
  highp float dissipation;
  highp vec2 uTexelSize;
};

uniform sampler2D velocityTexture;
uniform float amount;

in vec2 texturePosition;
out vec2 outVelocity;

void main() {
  vec2 velocity = texture(velocityTexture, texturePosition).xy;
  vec2 advectedCoord = texturePosition - amount * velocity;
  float decay = 1.0 + dissipation * amount;
  outVelocity = texture(velocityTexture, advectedCoord).xy / decay;
}
