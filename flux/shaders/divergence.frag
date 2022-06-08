#ifdef GL_ES
precision mediump float;
precision highp sampler2D;
#endif

layout(std140) uniform FluidUniforms
{
  highp float deltaT;
  highp float dissipation;
  highp vec2 uTexelSize;
};

uniform sampler2D velocityTexture;

in highp vec2 texturePosition;
out float newDivergence;

void main() {
  float L = textureOffset(velocityTexture, texturePosition, ivec2(-1, 0)).x;
  float R = textureOffset(velocityTexture, texturePosition, ivec2(1, 0)).x;
  float T = textureOffset(velocityTexture, texturePosition, ivec2(0, 1)).y;
  float B = textureOffset(velocityTexture, texturePosition, ivec2(0, -1)).y;

  vec2 velocity = texture(velocityTexture, texturePosition).xy;
  if (texturePosition.x <= uTexelSize.x)       { L = -velocity.x; }
  if (texturePosition.x >= 1.0 - uTexelSize.x) { R = -velocity.x; }
  if (texturePosition.y >= 1.0 - uTexelSize.y) { T = -velocity.y; }
  if (texturePosition.y <= uTexelSize.y)       { B = -velocity.y; }

  newDivergence = 0.5 * ((R - L) + (T - B));
}
