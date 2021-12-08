#version 300 es
precision mediump float;

in vec2 vPosition;
in float vSize;
in float vAngle;
in float vTotalOpacity;

uniform vec3 uColor;

out vec4 fragColor;

void main() {
  fragColor = vec4(uColor, 0.9 * vTotalOpacity);
}
