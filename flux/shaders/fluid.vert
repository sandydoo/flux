precision highp float;

layout(std140) uniform FluidUniforms
{
  highp float deltaT;
  highp float epsilon;
  highp float halfEpsilon;
  highp float dissipation;
  highp vec2 uTexelSize;
  lowp float pad1;
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
