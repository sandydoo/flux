#ifdef GL_ES
precision mediump float;
precision highp sampler2D;
#endif

uniform sampler2D velocityTexture;

in highp vec2 texturePosition;
out float newDivergence;

void main() {
  float L = textureOffset(velocityTexture, texturePosition, ivec2(-1, 0)).x;
  float R = textureOffset(velocityTexture, texturePosition, ivec2(1, 0)).x;
  float T = textureOffset(velocityTexture, texturePosition, ivec2(0, 1)).y;
  float B = textureOffset(velocityTexture, texturePosition, ivec2(0, -1)).y;

  newDivergence = 0.5 * ((R - L) + (T - B));
}
