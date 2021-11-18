#version 300 es

precision highp float;
precision highp sampler2D;

uniform float deltaT;
uniform sampler2D inputTexture;
uniform sampler2D noiseTexture;

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
  float C = texture(noiseTexture, textureCoord).x;
  vec2 force = 0.5 * vec2(abs(T) - abs(B), abs(R) - abs(L));
  force /= length(force) + 0.0001;
  force *= 0.5 * C;
  force.y *= -1.0;

  vec2 inputValue = texture(inputTexture, textureCoord).xy;
  inputValue += force * deltaT;

  fragColor = vec4(inputValue, 0.0, 1.0);
}
