precision highp float;

in vec2 lineVertex;
in vec2 basepoint;


in vec2 iEndpointVector;
in vec2 iVelocityVector;
in mediump vec4 iColor;
in mediump float iLineWidth;
in mediump float iLineOpacity;
in mediump float iEndpointOpacity;

layout(std140) uniform Projection
{
  mat4 uProjection;
  mat4 uView;
};

layout(std140) uniform LineUniforms
{
  mediump float uLineWidth;
  mediump float uLineLength;
  mediump float uLineBeginOffset;
  mediump float uLineFadeOutLength;
};

out vec2 vVertex;
out vec3 vColor;
out float vOpacity;

void main() {
  vec2 endpoint = basepoint + iEndpointVector * uLineLength;

  vec2 xBasis = endpoint - basepoint;
  vec2 yBasis = normalize(vec2(-xBasis.y, xBasis.x));
  vec2 point = basepoint + xBasis * lineVertex.x + yBasis * (iLineWidth * uLineWidth) * lineVertex.y;

  gl_Position = uProjection * uView * vec4(point, 0.0, 1.0);

  vVertex = lineVertex;
  vColor = iColor.rgb;
  vOpacity = iLineOpacity;
}
