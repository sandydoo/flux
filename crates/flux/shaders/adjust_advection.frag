precision mediump float;
precision highp sampler2D;

in vec2 texturePosition;

uniform sampler2D velocityTexture;
uniform sampler2D forwardAdvectedTexture;
uniform sampler2D reverseAdvectedTexture;
uniform float deltaTime;

out vec2 outVelocity;

void main() {
  vec2 size = vec2(textureSize(velocityTexture, 0));
  ivec2 position = ivec2(texturePosition * size);
  vec2 velocity = texelFetch(velocityTexture, position, 0).xy;

  vec2 newCoord = (0.5 + floor((vec2(position) + 1.0) - deltaTime * velocity)) / size;
  vec2 L = textureOffset(velocityTexture, newCoord, ivec2(-1, 0)).xy;
  vec2 R = textureOffset(velocityTexture, newCoord, ivec2(1, 0)).xy;
  vec2 T = textureOffset(velocityTexture, newCoord, ivec2(0, 1)).xy;
  vec2 B = textureOffset(velocityTexture, newCoord, ivec2(0, -1)).xy;

  vec2 minVelocity = min(L, min(R, min(T, B)));
  vec2 maxVelocity = max(L, max(R, max(T, B)));

  vec2 forward = texelFetch(forwardAdvectedTexture, position, 0).xy;
  vec2 reverse = texelFetch(reverseAdvectedTexture, position, 0).xy;

  vec2 adjustedVelocity = forward + 0.5 * (velocity - reverse);
  outVelocity = clamp(adjustedVelocity, minVelocity, maxVelocity);
}
