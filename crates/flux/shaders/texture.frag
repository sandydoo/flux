precision mediump float;
precision highp sampler2D;

in vec2 texturePosition;
uniform sampler2D inputTexture;
out vec4 fragColor;

void main() {
  fragColor = vec4(texture(inputTexture, texturePosition).rgb * 0.5 + 0.5, 1.0);
}
