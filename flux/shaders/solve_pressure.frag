#ifdef GL_ES
precision mediump float;
precision highp sampler2D;
#endif

uniform float rBeta;
uniform float alpha;
uniform sampler2D divergenceTexture;
uniform sampler2D pressureTexture;

in highp vec2 texturePosition;
out float outPressure;

void main() {
  vec2 size = vec2(textureSize(divergenceTexture, 0));
  ivec2 texelPosition = ivec2(floor(size * texturePosition));
  float pressure = texelFetch(pressureTexture, texelPosition, 0).x;
  float divergence = texelFetch(divergenceTexture, texelPosition, 0).x;

  float L = textureOffset(pressureTexture, texturePosition, ivec2(-1, 0)).x;
  float R = textureOffset(pressureTexture, texturePosition, ivec2(1, 0)).x;
  float T = textureOffset(pressureTexture, texturePosition, ivec2(0, 1)).x;
  float B = textureOffset(pressureTexture, texturePosition, ivec2(0, -1)).x;

  outPressure = rBeta * (L + R + B + T + alpha * divergence);
}
