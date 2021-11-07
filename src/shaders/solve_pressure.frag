#version 300 es
precision highp float;
precision highp sampler2D;

uniform float rBeta;
uniform float alpha;
uniform sampler2D divergenceTexture;
uniform sampler2D pressureTexture;

in vec2 textureCoord;
in vec2 vL;
in vec2 vR;
in vec2 vT;
in vec2 vB;
out vec4 fragColor;

void main() {
  float L = texture(pressureTexture, vL).x;
  float R = texture(pressureTexture, vR).x;
  float T = texture(pressureTexture, vT).x;
  float B = texture(pressureTexture, vB).x;
  float divergence = texture(divergenceTexture, textureCoord).x;

  float pressure = rBeta * (L + R + B + T + alpha * divergence);
  fragColor = vec4(pressure, 0.0, 0.0, 1.0);
}
