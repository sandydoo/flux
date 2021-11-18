#version 300 es
precision highp float;
precision highp sampler2D;

in vec3 position;
out vec2 textureCoord;

void main() {
  gl_Position = vec4(position, 1.0);
  textureCoord = position.xy * 0.5 + 0.5;
}
