#version 300 es

precision highp float;
precision highp sampler2D;

in vec2 lineVertex;
in vec2 basepoint;

in vec2 iEndpointVector;
in vec2 iVelocityVector;
in float iLineWidth;
in vec3 iColor;

uniform float uLineWidth;
uniform float uLineLength;
uniform mat4 uProjection;

out vec2 vVertex;
out vec3 vColor;
out float vTotalOpacity;

mat4 scale(vec3 v) {
  return mat4(
    v.x, 0.0, 0.0, 0.0,
    0.0, v.y, 0.0, 0.0,
    0.0, 0.0, v.z, 0.0,
    0.0, 0.0, 0.0, 1.0
  );
}

void main() {
  vec2 endpoint = basepoint + iEndpointVector * uLineLength;

  vec2 xBasis = endpoint - basepoint;
  vec2 yBasis = normalize(vec2(-xBasis.y, xBasis.x));
  vec2 point = basepoint + xBasis * lineVertex.x + yBasis * (iLineWidth * uLineWidth) * lineVertex.y;

  gl_Position = uProjection * vec4(point, 0.0, 1.0);

  vVertex = lineVertex;
  vColor = iColor;
  vTotalOpacity = smoothstep(30.0, 80.0, length(endpoint - basepoint));
}
