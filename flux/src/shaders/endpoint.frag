#version 300 es
precision mediump float;

in vec2 vPosition;
in float vSize;
in vec3 vColor;
in float vTotalOpacity;

uniform float uLineOpacity;

out vec4 fragColor;

void main() {
  fragColor = vec4(vColor, uLineOpacity * vTotalOpacity);
}
