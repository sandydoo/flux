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
uniform sampler2D pressureTexture;

in vec2 texturePosition;
in vec2 vL;
in vec2 vR;
in vec2 vT;
in vec2 vB;
out vec2 newVelocity;

void main() {
  float pressure = texture(pressureTexture, texturePosition).x;
  float L = texture(pressureTexture, vL).x;
  float R = texture(pressureTexture, vR).x;
  float T = texture(pressureTexture, vT).x;
  float B = texture(pressureTexture, vB).x;

  vec2 adjustment = vec2(1.0);
  if (texturePosition.x == 0.0) {
    adjustment.x = 0.0;
    L = pressure;
  }
  if (texturePosition.x == 1.0) {
    adjustment.x = 0.0;
    R = pressure;
  }
  if (texturePosition.y == 0.0) {
    adjustment.y = 0.0;
    B = pressure;
  }
  if (texturePosition.y == 1.0) {
    adjustment.y = 0.0;
    T = pressure;
  }

  vec2 velocity = texture(velocityTexture, texturePosition).xy;
  newVelocity = adjustment * (velocity - 0.5 * vec2(R - L, T - B));
}
