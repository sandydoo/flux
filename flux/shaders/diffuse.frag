#ifdef GL_ES
precision mediump float;
precision highp sampler2D;
#endif

uniform float alpha;
uniform float rBeta;
uniform sampler2D velocityTexture;

in vec2 texturePosition;
out vec2 outVelocity;

void main() {
  vec2 L = textureOffset(velocityTexture, texturePosition, ivec2(-1, 0)).xy;
  vec2 R = textureOffset(velocityTexture, texturePosition, ivec2(1, 0)).xy;
  vec2 T = textureOffset(velocityTexture, texturePosition, ivec2(0, 1)).xy;
  vec2 B = textureOffset(velocityTexture, texturePosition, ivec2(0, -1)).xy;
  vec2 velocity = texture(velocityTexture, texturePosition).xy;

  outVelocity = rBeta * (L + R + B + T + alpha * velocity);
}
