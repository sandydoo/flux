#define PI 3.1415926535897932384626433832795
#ifdef GL_ES
precision highp float;
#endif

layout(std140) uniform LineUniforms
{
  highp float aspect;
  highp float zoom;
  highp float uLineWidth;
  highp float uLineLength;
  mediump float uLineBeginOffset;
  mediump float uLineVariance;
  mediump vec2 lineNoiseScale;
  mediump float lineNoiseOffset1;
  mediump float lineNoiseOffset2;
  mediump float lineNoiseBlendFactor;
  highp float deltaTime;
};

uniform float uOrientation;

in vec2 vertex;
in vec2 basepoint;

in highp vec2 iEndpointVector;
in mediump vec2 iVelocityVector;
in mediump vec4 iColor;
in mediump float iLineWidth;

out vec2 vVertex;
out vec2 vMidpointVector;
out mediump vec4 vTopColor;
out mediump vec4 vBottomColor;

void main() {
  vec2 point
    = vec2(aspect, 1.0) * zoom * (basepoint * 2.0 - 1.0)
    + iEndpointVector
    + 0.5 * uLineWidth * iLineWidth * vertex;
  point.x /= aspect;

  gl_Position = vec4(point, 0.0, 1.0);
  vVertex = vertex;

  // Rotate the endpoint vector 90°. We use this to detect which side of the
  // endpoint we’re on in the fragment.
  vMidpointVector = vec2(iEndpointVector.y, -iEndpointVector.x);

  float endpointOpacity = clamp(iColor.a + (1.0 - iColor.a), 0.0, 1.0);
  vTopColor = vec4(iColor.rgb, endpointOpacity);

  // The color of the lower half of the endpoint is less obvious. We’re
  // drawing over part of the line, so to match the color of the upper
  // endpoint, we have to do some math. Luckily, we know the premultiplied
  // color of the line underneath, so we can reverse the blend equation to get
  // the right color.
  //
  // GL_BLEND(SRC_ALPHA, ONE) = srcColor * srcAlpha + dstColor * srcAlpha
  // = vColor * vEndpointOpacity + vColor * vLineOpacity
  //
  // Remember, we’ve already premultiplied our colors! The opacity should be
  // 1.0 to disable more opacity blending!
  vec3 premultipliedLineColor = iColor.rgb * iColor.a;
  vBottomColor = vec4(iColor.rgb * endpointOpacity - premultipliedLineColor, 1.0);
}
