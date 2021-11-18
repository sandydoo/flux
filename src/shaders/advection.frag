#version 300 es
precision highp float;
precision highp sampler2D;

uniform float deltaT;
uniform float epsilon;
uniform float dissipation;
uniform sampler2D inputTexture;
uniform sampler2D velocityTexture;

in vec2 textureCoord;
out vec4 fragColor;

void main() {
  vec2 offset = vec2(0.0, 0.0);
  vec2 scale = vec2(1.0, 1.0);

  if (textureCoord.x < 0.0) {
    offset.x = 1.0;
    scale.x = -1.0;
  } else if (textureCoord.x > 1.0) {
    offset.x = -1.0;
    scale.x = -1.0;
  }
  if (textureCoord.y < 0.0) {
    offset.y = 1.0;
    scale.y = -1.0;
  } else if (textureCoord.y > 1.0) {
    offset.y = -1.0;
    scale.y = -1.0;
  }

  vec2 velocity = scale * texture(velocityTexture, textureCoord + offset).xy;

  vec2 pastCoord = textureCoord - (epsilon * deltaT * velocity);
  vec4 pastColor = texture(inputTexture, pastCoord);
  float decay = 1.0 + dissipation * deltaT;
  fragColor = pastColor / decay;
}
