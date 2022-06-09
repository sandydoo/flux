#ifdef GL_ES
precision highp float;
#endif

in vec2 lineVertex;
in vec2 basepoint;

in vec2 iEndpointVector;
in vec2 iVelocityVector;
in mediump vec4 iColor;
in mediump float iLineWidth;

layout(std140) uniform Projection
{
  mat4 uFluidProjection;
  mat4 uProjection;
  mat4 uView;
};

layout(std140) uniform LineUniforms
{
  mediump float uLineWidth;
  mediump float uLineLength;
  mediump float uLineBeginOffset;
};

out vec2 vVertex;
out vec4 vColor;

void main() {
  vec2 endpoint = basepoint + iEndpointVector * uLineLength;

  vec2 yBasis = endpoint - basepoint;
  vec2 xBasis = normalize(vec2(-yBasis.y, yBasis.x));
  vec2 point = basepoint + yBasis * lineVertex.y + xBasis * (iLineWidth * uLineWidth) * lineVertex.x;

  gl_Position = uProjection * uView * vec4(point, 0.0, 1.0);

  vVertex = lineVertex;
  vColor = iColor;
}
