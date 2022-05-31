#ifdef GL_ES
precision mediump float;
#endif

in vec2 vVertex;
in vec4 vColor;

out vec4 fragColor;

void main() {
  float distance = length(vVertex);
  float antialiasing = 1.0 - smoothstep(1.0 - fwidth(distance), 1.0, distance);
  fragColor = vec4(vColor.rgb, vColor.a * antialiasing);
}
