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
  vec2 noise = texture(noiseTexture, texturePosition).xy;
  outVelocity = velocity + deltaTime * noise;
}
