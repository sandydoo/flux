#version 300 es

precision highp float;
precision highp sampler2D;

in vec2 textureCoord;
uniform sampler2D inputTexture;
out vec4 fragColor;

void main() {
  fragColor = vec4(texture(inputTexture, textureCoord).rgb * 0.5 + 0.5, 1.0);
}