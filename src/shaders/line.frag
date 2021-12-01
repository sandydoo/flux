#version 300 es
precision mediump float;

in vec2 vVertex;
in float vHeight;

uniform vec3 uColor;

out vec4 fragColor;

void main() {
  float opacity = 0.9 * smoothstep(0.1, 0.15, vHeight) * smoothstep(0.4, 1.0, vVertex.x);
  fragColor = vec4(uColor, opacity);
}
