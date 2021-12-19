#version 300 es
#define PI 3.1415926535897932384626433832795
precision highp float;
precision highp sampler2D;

uniform float uLineWidth;
uniform float uLineLength;
uniform mat4 uProjection;

in vec2 vertex;
in vec2 basepoint;

in vec2 iEndpointVector;
in vec2 iVelocityVector;
in float iLineWidth;
in vec3 iColor;

out vec2 vPosition;
out vec3 vColor;
out float vSize;
out float vTotalOpacity;

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
  float height = length(endpoint - basepoint) / uLineLength;

  vec2 direction = normalize(endpoint - basepoint);
  float angle = -atan(direction.y, direction.x) + PI / 2.0;

  float pointSize = uLineWidth * width;
  mat4 modelMatrix = mat4(
    0.5 * pointSize, 0.0, 0.0, 0.0,
    0.0, 0.5 * pointSize, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0
  );

  gl_Position = uProjection * translate(vec3(endpoint, 0.0)) * rotateZ(angle) * modelMatrix * vec4(vertex, 0.0, 1.0);

  vPosition = vertex;
  vColor = iColor;
  vSize = height;
  vTotalOpacity = smoothstep(20.0, 50.0, length(endpoint - basepoint));
}
