#version 300 es
#define PI 3.1415926535897932384626433832795
precision mediump float;

in vec2 vertex;
in vec2 basepoint;

in vec2 iEndpointVector;
in vec2 iVelocityVector;
in float iLineWidth;
in vec4 iColor;
in float iOpacity;

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
  mediump float uLineBaseOpacity;
  mediump float uLineFadeOutLength;
  mediump float deltaT;
  mediump vec2 padding;
  mediump vec3 uColorWheel[6];
};

out vec2 vPosition;
out vec3 vColor;
out float vSize;
out float vOpacity;

mat4 translate(vec3 v) {
  return mat4(
    vec4(1.0, 0.0, 0.0, 0.0),
    vec4(0.0, 1.0, 0.0, 0.0),
    vec4(0.0, 0.0, 1.0, 0.0),
    vec4(v.x, v.y, v.z, 1.0)
  );
}

mat4 scale(vec3 v) {
  return mat4(
    v.x, 0.0, 0.0, 0.0,
    0.0, v.y, 0.0, 0.0,
    0.0, 0.0, v.z, 0.0,
    0.0, 0.0, 0.0, 1.0
  );
}

mat4 rotateZ(float angle) {
  float s = sin(angle);
  float c = cos(angle);

  return mat4(
    c,   -s,  0.0, 0.0,
    s,  c,    0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0
  );
}

// TODO: A lot of this shared with lines. Can we do something about that?
void main() {
  vec2 endpoint = basepoint + iEndpointVector * uLineLength;

  float width = iLineWidth;
  float height = length(iEndpointVector);

  vec2 direction = normalize(endpoint - basepoint);
  float angle = -atan(direction.y, direction.x) + PI / 2.0;

  float pointSize = uLineWidth * width;
  mat4 modelMatrix = mat4(
    0.5 * pointSize, 0.0, 0.0, 0.0,
    0.0, 0.5 * pointSize, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0
  );

  gl_Position = uProjection * uView * translate(vec3(endpoint, 0.0)) * rotateZ(angle) * modelMatrix * vec4(vertex, 0.0, 1.0);

  vPosition = vertex;
  vColor = iColor.rgb;
  vSize = height;
  vOpacity = iOpacity;
}
