precision mediump float;

in vec3 position;
uniform vec2 resolution;

out vec2 texCoord;
out vec2 v_rgbNW;
out vec2 v_rgbNE;
out vec2 v_rgbSW;
out vec2 v_rgbSE;
out vec2 v_rgbM;

void main() {
  gl_Position = vec4(position, 1.0);

  texCoord = (position.xy + 1.0) * 0.5;
  texCoord.y = 1.0 - texCoord.y;
  vec2 fragCoord = texCoord * resolution;

  vec2 inverseVP = 1.0 / resolution.xy;
  v_rgbNW = (fragCoord + vec2(-1.0, -1.0)) * inverseVP;
  v_rgbNE = (fragCoord + vec2(1.0, -1.0)) * inverseVP;
  v_rgbSW = (fragCoord + vec2(-1.0, 1.0)) * inverseVP;
  v_rgbSE = (fragCoord + vec2(1.0, 1.0)) * inverseVP;
  v_rgbM = vec2(fragCoord * inverseVP);
}
