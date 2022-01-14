#version 300 es

precision highp float;
precision highp sampler2D;

uniform float deltaT;
uniform float uMultiplier;
uniform float uBlendProgress;
uniform sampler2D inputTexture;
uniform mediump sampler2D noiseTexture;

in vec2 textureCoord;
out vec2 outputValue;

vec2 clockwisePerpendicular(in vec2 vector) {
  return vec2(vector.y, -vector.x);
}

vec2 anticlockwisePerpendicular(in vec2 vector) {
  return vec2(-vector.y, vector.x);
}

void main() {
  float noise = texture(noiseTexture, textureCoord).x;
  vec2 inputValue = texture(inputTexture, textureCoord).xy;

  vec2 direction = normalize(inputValue);
  vec2 clockwise = clockwisePerpendicular(direction);
  vec2 force = clockwise * noise;

  outputValue = inputValue + uBlendProgress * uMultiplier * force;
}
