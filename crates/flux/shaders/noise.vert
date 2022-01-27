precision highp float;

layout(std140) uniform NoiseUniforms
{
  highp float uFrequency;
  highp float uOffset1;
  highp float uOffset2;
  highp float uMultiplier;
  highp vec2 uTexelSize;
  highp float uBlendThreshold;
  lowp float pad2;
};

in vec3 position;

out vec2 textureCoord;
out vec2 vL;
out vec2 vR;
out vec2 vT;
out vec2 vB;

void main() {
  gl_Position = vec4(position, 1.0);
  textureCoord = position.xy * 0.5 + 0.5;

  vL = textureCoord - vec2(uTexelSize.x, 0.0);
  vR = textureCoord + vec2(uTexelSize.x, 0.0);
  vT = textureCoord + vec2(0.0, uTexelSize.y);
  vB = textureCoord - vec2(0.0, uTexelSize.y);
}
