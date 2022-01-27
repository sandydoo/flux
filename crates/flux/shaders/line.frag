precision mediump float;

in vec2 vVertex;
in vec3 vColor;
in float vOpacity;

layout(std140) uniform LineUniforms
{
  highp float uLineWidth;
  highp float uLineLength;
  highp float uLineBeginOffset;
  highp float uLineFadeOutLength;
};

out vec4 fragColor;

void main() {
  float opacity = vOpacity * smoothstep(uLineBeginOffset, 1.0, vVertex.x);
  fragColor = vec4(vColor, opacity);
}
