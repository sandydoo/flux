#version 300 es
#define PI 3.1415926535897932384626433832795

precision highp float;
precision highp sampler2D;

in vec3 position;
in vec3 basepoint;
in vec4 color;

uniform float deltaT;
uniform sampler2D velocityTexture;

out vec4 vColor;

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

void main() {;
  vec2 textureCoord = basepoint.xy * 0.5 + 0.5;

  vec2 velocity = texture(velocityTexture, textureCoord).xy;
  float angle = -atan(velocity.y, velocity.x) + PI / 2.0; // figure out the right angle here
  float magnitude = length(velocity);
  gl_Position = translate(basepoint) * rotateZ(angle) * scale(vec3(magnitude, magnitude, 1.0)) * vec4(position, 1.0);

  vColor = color;
}
