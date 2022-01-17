#version 300 es
precision mediump float;

in vec2 vVertex;
in vec3 vColor;
in float vOpacity;

layout(std140) uniform LineUniforms
{
  mediump float uLineWidth;
  mediump float uLineLength;
  mediump float uLineBeginOffset;
  mediump float uLineBaseOpacity;
  mediump float uLineFadeOutLength;
  mediump float deltaT;
  mediump vec2 padding;
  mediump vec3 uColorWheel[6];
};

out vec4 fragColor;

void main() {
  float opacity = uLineBaseOpacity * vOpacity * smoothstep(uLineBeginOffset, 1.0, vVertex.x);
  fragColor = vec4(vColor, opacity);
}
