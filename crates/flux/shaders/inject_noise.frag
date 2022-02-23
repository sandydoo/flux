precision mediump float;
precision highp sampler2D;

uniform float deltaTime;
uniform sampler2D velocityTexture;
uniform sampler2D noiseTexture;

in vec2 textureCoord;
out vec2 outVelocity;

void main() {
  vec2 noise = texture(noiseTexture, textureCoord).xy;
  vec2 velocity = texture(velocityTexture, textureCoord).xy;
  outVelocity = velocity + deltaTime * noise;
}
