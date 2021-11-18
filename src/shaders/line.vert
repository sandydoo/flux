#version 300 es
#define PI 3.1415926535897932384626433832795

precision highp float;
precision highp sampler2D;

in vec3 vertex;

uniform float deltaT;
uniform uint lineCount;
uniform vec3 uColor;
uniform sampler2D lineStateTexture;

out vec3 vVertex;
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
  vec2 velocity = lineState.ba;

  // TODO: Maybe make this configurable. Can get quite different feels by
  // rotating the lines relative to the velocity field.
  float angle = -atan(velocity.y, velocity.x) - PI / 2.0;

  // TODO: Think through the scaling here. Make it configurable.
  float width = smoothstep(0.0, 0.2, length(velocity));
  float height = length(velocity);

  // TODO: actually make this a uniform
  mat4 uViewMatrix = scale(vec3(2.0));
  gl_Position = uViewMatrix * translate(vec3(position.xy, 0.0)) * rotateZ(angle) * scale(vec3(width, height, 1.0)) * vec4(vertex, 1.0);

  vVertex = vertex;
  vHeight = height;
}
