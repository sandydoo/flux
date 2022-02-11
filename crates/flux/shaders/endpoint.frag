precision highp float;

in vec2 vPosition;
in vec3 vColor;
in vec3 vPremultipliedLineColor;
in float vEndpointOpacity;
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
  // Figure out which side of the endpoint we’re on. Center is 0,0.
  // sign((B.x - center.x) * (y - center.y) - (B.y - center.y) * (x - center.x));
  float sideOfEndpoint = vPerpendicularVector.x * vPosition.y - vPerpendicularVector.y * vPosition.x;
  bool isUpperEndpoint = sideOfEndpoint >= 0.0 ? true : false;

  vec4 upperHalfColor = vec4(vColor, vEndpointOpacity);

  // The color of the lower half of the endpoint is less obvious. We’re drawing
  // over part of the line, so, to match the color of the upper endpoint, we
  // have to do some math. Luckily, we know the premultiplied color of the line
  // underneath, so we can reverse the blend equation to get the right color.
  //
  // GL_BLEND(SRC_ALPHA, ONE) = srcColor * srcAlpha + dstColor * srcAlpha
  // = vColor * vEndpointOpacity + vColor * vLineOpacity
  //
  // Remember, we’ve already premultiplied our colors! The opacity should be 1.0
  // to disable more opacity blending!
  vec4 lowerHalfColor = vec4(vColor * vEndpointOpacity - vPremultipliedLineColor, 1.0);

  if (isUpperEndpoint) {
    fragColor = upperHalfColor;
  } else {
    fragColor = lowerHalfColor;
  }
}
