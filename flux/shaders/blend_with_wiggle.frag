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
out vec2 outputValue;

vec2 clockwisePerpendicular(in vec2 vector) {
  return vec2(vector.y, -vector.x);
}

vec2 anticlockwisePerpendicular(in vec2 vector) {
  return vec2(-vector.y, vector.x);
}

void main() {
  float noise = texture(noiseTexture, textureCoord).x;
  vec2 inputValue = texture(inputTexture, textureCoord).xy;

  vec2 direction = normalize(inputValue);
  vec2 clockwise = clockwisePerpendicular(direction);
  vec2 force = clockwise * noise;

  outputValue = inputValue + uBlendProgress * uMultiplier * force;
}
