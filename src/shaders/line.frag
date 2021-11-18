#version 300 es
precision highp float;

in vec3 vVertex;
in float vHeight;

uniform vec3 uColor;

out vec4 fragColor;

void main() {
  float opacity = smoothstep(0.2, 1.0, vVertex.y);
  fragColor = vec4(uColor, smoothstep(0.05, 0.1, vHeight) * 0.9 * opacity);
}
