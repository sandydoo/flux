#version 300 es
#define PI 3.1415926535897932384626433832795
precision highp float;
precision highp sampler2D;

uniform float uLineWidth;
uniform float uLineLength;
uniform mat4 uProjection;
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
  vec2 velocityVector = lineState.ba;

  // TODO: Think through the scaling here. Make it configurable.
  float velocity = length(velocityVector);
  float width = smoothstep(0.0, 0.2, velocity);
  float height = smoothstep(0.0, 0.2, velocity);

  vec2 direction = normalize(velocityVector);
  direction.y *= -1.0;
  vec2 endPoint = position - (direction * uLineLength * height);

  float angle = atan(velocityVector.y, velocityVector.x) - PI / 2.0;

  mat4 uViewMatrix = scale(vec3(1.6));
  float pointSize = uLineWidth * width;
  mat4 modelMatrix = mat4(
    0.5 * pointSize, 0.0, 0.0, 0.0,
    0.0, 0.5 * pointSize, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0
  );

  gl_Position = uViewMatrix * uProjection * translate(vec3(endPoint, 0.0)) * modelMatrix * vec4(vertex, 0.0, 1.0);

  vPosition = vertex;
  vSize = height;
  vAngle = angle;
}
