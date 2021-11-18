#version 300 es
precision highp float;
precision highp sampler2D;

uniform float epsilon;
uniform sampler2D velocityTexture;
uniform sampler2D pressureTexture;

in vec2 textureCoord;
in vec2 vL;
in vec2 vR;
in vec2 vT;
in vec2 vB;
out vec4 fragColor;

void main() {
  vec2 velocity = texture(velocityTexture, textureCoord).xy;

  float L = texture(pressureTexture, vL).x;
  float R = texture(pressureTexture, vR).x;
  float T = texture(pressureTexture, vT).x;
  float B = texture(pressureTexture, vB).x;

  velocity.xy -= 0.5 * epsilon * vec2(R - L, T - B);
  fragColor = vec4(velocity, 0.0, 1.0);
}
