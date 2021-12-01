#version 300 es
precision mediump float;

in vec2 vPosition;
in float vSize;
in float vAngle;

uniform vec3 uColor;

out vec4 fragColor;

void main() {
  vec2 center = vec2(0.0, 0.0);
  float centerDist = length(vPosition);
  float x = vPosition.x;
  float y = vPosition.y;

  vec3 N = vec3(sin(vAngle), cos(vAngle), 0.0);
  vec3 B = cross(N, vec3(0.0, 0.0, 1.0));
  float side = sign((B.x - center.x) * (y - center.y) - (B.y - center.y) * (x - center.x));

  // Draw the entire endpoint when the line is small. Otherwise, draw only half.
  if (side >= 0.0 || vSize < 0.1) {
    fragColor = vec4(uColor, 0.9);
  } else {
    fragColor = vec4(uColor.rgb, 0.9 * (1.0 - smoothstep(0.05, 0.1, vSize)));
  }
}
