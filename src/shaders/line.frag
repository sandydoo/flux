#version 300 es
precision mediump float;

in vec2 vVertex;
in float vTotalOpacity;

uniform float uLineBeginOffset;
uniform vec3 uColor;

out vec4 fragColor;

void main() {
  float opacity = 0.9 * vTotalOpacity * smoothstep(uLineBeginOffset, 1.0, vVertex.x);
  fragColor = vec4(uColor, opacity);
}
