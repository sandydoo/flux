#ifdef GL_ES
precision mediump float;
#endif

in vec3 position;
out vec2 clipPosition;
out vec2 texturePosition;

void main() {
  gl_Position = vec4(position, 1.0);
  texturePosition = 0.5 + 0.5 * position.xy;
  clipPosition = position.xy;
}
