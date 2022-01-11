#version 300 es
precision mediump float;

in vec2 vPosition;
in float vSize;
in vec3 vColor;
in float vOpacity;

uniform float uLineBaseOpacity;

out vec4 fragColor;

void main() {
  fragColor = vec4(vColor, uLineBaseOpacity * vOpacity);
}
