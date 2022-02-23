precision mediump float;
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
out float outPressure;

void main() {
  float L = texture(pressureTexture, vL).x;
  float R = texture(pressureTexture, vR).x;
  float T = texture(pressureTexture, vT).x;
  float B = texture(pressureTexture, vB).x;
  float divergence = texture(divergenceTexture, textureCoord).x;

  outPressure = rBeta * (L + R + B + T + alpha * divergence);
}
