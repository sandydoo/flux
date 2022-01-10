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

out vec2 newVelocity;

void main () {
  float L = texture(velocityTexture, vL).y;
  float R = texture(velocityTexture, vR).y;
  float T = texture(velocityTexture, vT).x;
  float B = texture(velocityTexture, vB).x;
  float vorticity = (R - L) - (T - B);
  newVelocity = texture(velocityTexture, textureCoord).xy + deltaT * vorticity;
}
