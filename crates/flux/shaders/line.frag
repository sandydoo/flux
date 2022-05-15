precision mediump float;

in vec2 vVertex;
in vec3 vColor;
in float vOpacity;

layout(std140) uniform LineUniforms
{
  mediump float uLineWidth;
  mediump float uLineLength;
  mediump float uLineBeginOffset;
};

out vec4 fragColor;

void main() {
  float opacity = vOpacity * smoothstep(uLineBeginOffset, 1.0, vVertex.x);
  fragColor = vec4(vColor, opacity);
}
