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
out vec2 newVelocity;

void main() {
  float pressure = texture(pressureTexture, texturePosition).x;
  float L = textureOffset(pressureTexture, texturePosition, ivec2(-1, 0)).x;
  float R = textureOffset(pressureTexture, texturePosition, ivec2(1, 0)).x;
  float T = textureOffset(pressureTexture, texturePosition, ivec2(0, 1)).x;
  float B = textureOffset(pressureTexture, texturePosition, ivec2(0, -1)).x;

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
