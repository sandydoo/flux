#ifdef GL_ES
precision mediump float;
#endif

in vec2 vVertex;
in vec2 vMidpointVector;
in vec4 vTopColor;
in vec4 vBottomColor;

out vec4 fragColor;

void main() {
  vec4 color = vBottomColor;

  // Test which side of the endpoint we’re on.
  float side
    = (vVertex.x - vMidpointVector.x) * (-vMidpointVector.y)
    - (vVertex.y - vMidpointVector.y) * (-vMidpointVector.x);

  if (side > 0.0) {
    color = vTopColor;
  }

  float distance = length(vVertex);
  float smoothEdges = 1.0 - smoothstep(1.0 - fwidth(distance), 1.0, distance);
  fragColor = vec4(color.rgb, color.a * smoothEdges);
}
