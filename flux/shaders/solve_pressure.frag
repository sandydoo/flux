#ifdef GL_ES
precision mediump float;
precision highp sampler2D;
#endif

uniform float rBeta;
uniform float alpha;
uniform sampler2D divergenceTexture;
uniform sampler2D pressureTexture;

in vec2 texturePosition;
out float outPressure;

void main() {
  float pressure = texture(pressureTexture, texturePosition).x;
  float L = textureOffset(pressureTexture, texturePosition, ivec2(-1, 0)).x;
  float R = textureOffset(pressureTexture, texturePosition, ivec2(1, 0)).x;
  float T = textureOffset(pressureTexture, texturePosition, ivec2(0, 1)).x;
  float B = textureOffset(pressureTexture, texturePosition, ivec2(0, -1)).x;

  if (texturePosition.x == 0.0) { L = pressure; }
  if (texturePosition.x == 1.0) { R = pressure; }
  if (texturePosition.y == 0.0) { B = pressure; }
  if (texturePosition.y == 1.0) { T = pressure; }

  float divergence = texture(divergenceTexture, texturePosition).x;
  outPressure = rBeta * (L + R + B + T + alpha * divergence);
}
