#version 300 es

precision highp float;
precision highp sampler2D;

uniform float deltaT;
uniform float uMultiplier;
uniform float uBlendProgress;
uniform sampler2D inputTexture;
uniform mediump sampler2D noiseTexture;

in vec2 textureCoord;
in highp vec2 vL;
in highp vec2 vR;
in highp vec2 vT;
in highp vec2 vB;
out vec4 fragColor;

// Add noise to a field with curl
void main() {
  float L = texture(noiseTexture, vL).y;
  float R = texture(noiseTexture, vR).y;
  float T = texture(noiseTexture, vT).x;
  float B = texture(noiseTexture, vB).x;
  vec2 C = texture(noiseTexture, textureCoord).xy;
  vec2 force = 0.5 * vec2(abs(T) - abs(B), abs(R) - abs(L));
  force /= length(force) + 0.0001;
  force *= 0.5 * C;

  vec2 inputValue = texture(inputTexture, textureCoord).xy;
  inputValue += uBlendProgress * uMultiplier * force;

  fragColor = vec4(inputValue, 0.0, 1.0);
}
