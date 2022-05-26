#ifdef GL_ES
precision mediump float;
precision highp sampler2D;
#endif

in vec2 texturePosition;
uniform sampler2D inputTexture;
out vec4 fragColor;

const float contrastFactor = 2.0;

void main() {
  vec3 color = 0.5 + 0.5 * texture(inputTexture, texturePosition).rgb;
  fragColor = vec4(clamp(contrastFactor * (color - 0.5) + 0.5, 0.0, 1.0), 1.0);
}
