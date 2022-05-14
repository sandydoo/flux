precision mediump float;
precision highp sampler2D;

in vec2 texturePosition;
uniform sampler2D inputTexture;
out vec4 fragColor;

const float contractFactor = 2.0;

void main() {
  vec3 color = 0.5 + 0.5 * texture(inputTexture, texturePosition).rgb;
  fragColor = vec4(clamp(contractFactor * (color - 0.5) + 0.5, 0.0, 1.0), 1.0);
}
