#version 300 es
#define PI 3.1415926535897932384626433832795
precision highp float;
precision highp sampler2D;

uniform sampler2D lineStateTexture;

in vec2 vertex;

out vec2 vPosition;
out float vSize;
out float vAngle;

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

vec4 getValueByIndexFromTexture(sampler2D tex, int index) {
  int texWidth = textureSize(tex, 0).x;
  int col = index % texWidth;
  int row = index / texWidth;
  return texelFetch(tex, ivec2(col, row), 0);
}

// TODO: A lot of this shared with lines. Can we do something about that?
void main() {
  vec4 lineState = getValueByIndexFromTexture(lineStateTexture, gl_InstanceID);
  vec2 position = lineState.rg;
  vec2 velocity = lineState.ba;

  float angle = -atan(velocity.y, velocity.x) - PI / 2.0;

  float width = smoothstep(0.0, 0.2, length(velocity)) * 0.005;
  float height = length(velocity);

  mat4 uViewMatrix = scale(vec3(2.0));
  vec2 translation = position + vec2(height * sin(angle), height * cos(angle));
  gl_Position = uViewMatrix * translate(vec3(translation, 0.0)) * scale(vec3(width, width, 1.0)) * vec4(vertex, 0.0, 1.0);

  vPosition = vertex;
  vSize = height;
  vAngle = angle;
}
