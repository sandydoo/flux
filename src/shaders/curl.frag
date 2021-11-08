#version 300 es
precision mediump float;
precision mediump sampler2D;

uniform float deltaT;
uniform sampler2D velocityTexture;

in vec2 textureCoord;
in highp vec2 vL;
in highp vec2 vR;
in highp vec2 vT;
in highp vec2 vB;

out vec4 fragColor;

void main () {
  float L = texture(velocityTexture, vL).y;
  float R = texture(velocityTexture, vR).y;
  float T = texture(velocityTexture, vT).x;
  float B = texture(velocityTexture, vB).x;
  float vorticity = R - L - T + B;
  fragColor = vec4(texture(velocityTexture, textureCoord).rg + deltaT * vorticity, 0.0, 1.0);
}
