precision highp float;
precision highp sampler2D;

layout(std140) uniform NoiseUniforms {
  highp float uFrequency;
  highp float uOffset1;
  highp float uOffset2;
  highp float uMultiplier;
  highp vec2 uTexelSize;
  highp float uBlendThreshold;
  lowp float pad2;
};

uniform float uBlendProgress;

uniform sampler2D inputTexture;
uniform sampler2D noiseTexture;

in vec2 textureCoord;
in highp vec2 vL;
in highp vec2 vR;
in highp vec2 vT;
in highp vec2 vB;
out vec2 outputValue;

// Add noise to a field with curl
void main() {
  float L = texture(noiseTexture, vL).y;
  float R = texture(noiseTexture, vR).y;
  float T = texture(noiseTexture, vT).x;
  float B = texture(noiseTexture, vB).x;
  vec2 force = vec2(abs(T) - abs(B), abs(L) - abs(R));
  // vec2 force =  vec2(R - L, T - B); // magnetic flowers
  force /= length(force) + 0.0001;

  if (length(force) < uBlendThreshold) {
    force *= 0.0;
  }

  vec2 inputValue = texture(inputTexture, textureCoord).xy;
  outputValue = inputValue + uBlendProgress * uMultiplier * force;
}
