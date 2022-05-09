precision mediump float;
precision highp sampler2D;

uniform float alpha;
uniform float rBeta;
uniform sampler2D velocityTexture;

in vec2 texturePosition;
in vec2 vL;
in vec2 vR;
in vec2 vT;
in vec2 vB;
out vec2 outVelocity;

void main() {
  vec2 L = texture(velocityTexture, vL).xy;
  vec2 R = texture(velocityTexture, vR).xy;
  vec2 T = texture(velocityTexture, vT).xy;
  vec2 B = texture(velocityTexture, vB).xy;
  vec2 velocity = texture(velocityTexture, texturePosition).xy;

  outVelocity = rBeta * (L + R + B + T + alpha * velocity);
}
