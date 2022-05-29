#ifdef GL_ES
precision mediump float;
precision highp sampler2D;
#endif

uniform float rBeta;
uniform float alpha;
uniform sampler2D divergenceTexture;
uniform sampler2D pressureTexture;

in vec2 texturePosition;
in vec2 vL;
in vec2 vR;
in vec2 vT;
in vec2 vB;
out float outPressure;

void main() {
  float pressure = texture(pressureTexture, texturePosition).x;
  float L = texture(pressureTexture, vL).x;
  float R = texture(pressureTexture, vR).x;
  float T = texture(pressureTexture, vT).x;
  float B = texture(pressureTexture, vB).x;

  if (texturePosition.x == 0.0) { L = pressure; }
  if (texturePosition.x == 1.0) { R = pressure; }
  if (texturePosition.y == 0.0) { B = pressure; }
  if (texturePosition.y == 1.0) { T = pressure; }

  float divergence = texture(divergenceTexture, texturePosition).x;
  outPressure = rBeta * (L + R + B + T + alpha * divergence);
}
