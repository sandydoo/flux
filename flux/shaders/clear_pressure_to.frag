#ifdef GL_ES
precision mediump float;
#endif

uniform float uStartingPressure;
out float pressure;

void main() {
  pressure = uStartingPressure;
}
