precision mediump float;
precision mediump sampler2D;

uniform float rBeta;
uniform float alpha;
uniform sampler2D divergenceTexture;
uniform sampler2D pressureTexture;

in vec2 textureCoord;
in vec2 vL;
in vec2 vR;
in vec2 vT;
in vec2 vB;
out vec2 newPressure;

void main() {
  vec2 L = texture(pressureTexture, vL).xy;
  vec2 R = texture(pressureTexture, vR).xy;
  vec2 T = texture(pressureTexture, vT).xy;
  vec2 B = texture(pressureTexture, vB).xy;
  vec2 divergence = texture(divergenceTexture, textureCoord).xy;

  newPressure = rBeta * (L + R + B + T + alpha * divergence);
}
