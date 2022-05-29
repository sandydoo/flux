#ifdef GL_ES
precision mediump float;
precision highp sampler2D;
#endif

uniform float deltaTime;
uniform sampler2D velocityTexture;
uniform sampler2D noiseTexture;

in vec2 texturePosition;
out vec2 outVelocity;

void main() {
  vec2 velocity = texture(velocityTexture, texturePosition).xy;
  float dx = 1.0 / float(textureSize(velocityTexture, 0));
  vec2 noise = texture(noiseTexture, texturePosition + 0.5 * dx).xy;
  outVelocity = velocity + deltaTime * noise;
}
