#version 300 es
precision mediump float;

in vec2 vVertex;
in vec3 vColor;
in float vOpacity;

uniform float uLineBaseOpacity;
uniform float uLineBeginOffset;

out vec4 fragColor;

void main() {
  float opacity = uLineBaseOpacity * vOpacity * smoothstep(uLineBeginOffset, 1.0, vVertex.x);
  fragColor = vec4(vColor, opacity);
}
