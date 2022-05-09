precision mediump float;
precision highp sampler2D;

uniform float deltaTime;
uniform sampler2D velocityTexture;
uniform sampler2D noiseTexture;

in vec2 texturePosition;
out vec2 outVelocity;

void main() {
  vec2 noise = texture(noiseTexture, texturePosition).xy;
  vec2 velocity = texture(velocityTexture, texturePosition).xy;
  outVelocity = velocity + deltaTime * noise;
}
