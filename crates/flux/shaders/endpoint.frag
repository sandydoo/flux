precision highp float;

in vec2 vPosition;
in vec3 vColor;
in vec3 vPremultipliedLineColor;
in float vOpacity;
in vec2 vPerpendicularVector;

layout(std140) uniform LineUniforms
{
  highp float uLineWidth;
  highp float uLineLength;
  highp float uLineBeginOffset;
  highp float uLineFadeOutLength;
};

out vec4 fragColor;

void main() {
  // sign((B.x - center.x) * (y - center.y) - (B.y - center.y) * (x - center.x));
  float sideOfEndpoint = vPerpendicularVector.x * vPosition.y - vPerpendicularVector.y * vPosition.x;
  bool isUpperEndpoint = sideOfEndpoint >= 0.0 ? true : false;

  vec4 upperHalfColor = vec4(vColor, vOpacity);
  vec4 lowerHalfColor = vec4(vColor - vPremultipliedLineColor, vOpacity);

  if (isUpperEndpoint) {
    fragColor = upperHalfColor;
  } else {
    fragColor = lowerHalfColor;
  }
}
