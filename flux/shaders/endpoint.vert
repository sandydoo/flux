#define PI 3.1415926535897932384626433832795
#ifdef GL_ES
precision highp float;
#endif

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
};

in vec2 vertex;
in vec2 basepoint;

in vec2 iEndpointVector;
in vec2 iVelocityVector;
in mediump vec4 iColor;
in mediump float iLineWidth;

out vec2 vVertex;
out vec4 vColor;
out float endpointOpacity;

mat4 translate(vec2 offset) {
  return mat4(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    offset.x, offset.y, 0.0, 1.0
  );
}

void main() {
  vec2 endpoint = basepoint + iEndpointVector * uLineLength;

  float angle = -atan(iEndpointVector.y, iEndpointVector.x) + PI / 2.0;
  float c = cos(angle);
  float s = sin(angle);
  mat4 rotationMatrix = mat4(
    c,   -s,  0.0, 0.0,
    s,   c,   0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0
  );

  float pointSize = uLineWidth * iLineWidth;
  mat4 modelMatrix = mat4(
    0.5 * pointSize, 0.0,                  0.0, 0.0,
    0.0,             0.5 * pointSize, 0.0, 0.0,
    0.0,             0.0,                  1.0, 0.0,
    0.0,             0.0,                  0.0, 1.0
  );

  gl_Position = uProjection * uView * translate(endpoint) * rotationMatrix * modelMatrix * vec4(vertex, 0.0, 1.0);

  vVertex = vertex;
  vColor = iColor;
  endpointOpacity = clamp(iColor.a + 0.7 * (1.0 - smoothstep(0.0, 0.75, iLineWidth)), 0.0, 1.0);
}
