in vec2 position;
out vec2 texturePosition;

void main() {
  // TODO: scale
  vec4 newPosition = vec4(position, 0.0, 1.0);
  gl_Position = newPosition;
  texturePosition = position * 0.5 + 0.5;
  texturePosition.y = 1.0 - texturePosition.y;
}
