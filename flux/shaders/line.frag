#ifdef GL_ES
precision mediump float;
#endif

in vec2 vVertex;
in vec4 vColor;
in float vLineOffset;

out vec4 fragColor;

void main() {
  float fade = smoothstep(vLineOffset, 1.0, vVertex.y);

  float xOffset = abs(vVertex.x);
  float smoothEdges = 1.0 - smoothstep(0.5 - fwidth(xOffset), 0.5, xOffset);

  fragColor = vec4(vColor.rgb, vColor.a * fade * smoothEdges);
}
