#ifdef GL_ES
precision mediump float;
#endif

uniform float uClearPressure;
out float pressure;

void main() {
  pressure = uClearPressure;
}
