#version 300 es
#define PI 3.1415926535897932384626433832795

precision highp float;
precision highp sampler2D;

in vec2 vertex;

uniform float deltaT;
uniform uint lineCount;
uniform vec3 uColor;
uniform mat4 uProjection;
uniform sampler2D lineStateTexture;

out vec2 vVertex;
out float vHeight;

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

vec4 getValueByIndexFromTexture(sampler2D tex, int index) {
  int texWidth = textureSize(tex, 0).x;
  int col = index % texWidth;
  int row = index / texWidth;
  return texelFetch(tex, ivec2(col, row), 0);
}

void main() {;
  vec4 lineState = getValueByIndexFromTexture(lineStateTexture, gl_InstanceID);
  vec2 position = lineState.rg;
  vec2 velocityVector = lineState.ba;

  // TODO: Think through the scaling here. Make it configurable.
  float velocity = length(velocityVector);
  float width = smoothstep(0.0, 0.2, velocity);
  float height = smoothstep(0.0, 0.2, velocity);

  float uLineLength = 400.0;
  float uLineWidth = 10.0;

  vec2 pointA = position;
  vec2 pointB = position + (normalize(velocityVector) * uLineLength * height);
  vec2 xBasis = pointB - pointA;
  xBasis.y *= -1.0; // flip y-axis
  vec2 yBasis = normalize(vec2(-xBasis.y, xBasis.x));
  vec2 point = pointA - xBasis * vertex.x - yBasis * (width * uLineWidth) * vertex.y;

  // TODO: actually make this a uniform
  mat4 uViewMatrix = scale(vec3(1.6));
  gl_Position = uViewMatrix * uProjection * vec4(point, 0.0, 1.0);

  vVertex = vertex;
  vHeight = height;
}
