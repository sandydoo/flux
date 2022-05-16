#define PI 3.1415926535897932384626433832795
#ifdef GL_ES
precision highp float;
#endif

layout(std140) uniform Projection
{
  mat4 uProjection;
  mat4 uView;
};

layout(std140) uniform LineUniforms
{
  mediump float uLineWidth;
  mediump float uLineLength;
  mediump float uLineBeginOffset;
};

uniform float uOrientation;

in vec2 vertex;
in vec2 basepoint;

in vec2 iEndpointVector;
in vec2 iVelocityVector;
in mediump vec4 iColor;
in mediump float iLineWidth;
in mediump float iLineOpacity;
in mediump float iEndpointOpacity;

out vec4 vColor;

mat4 translate(vec2 offset) {
  return mat4(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    offset.x, offset.y, 0.0, 1.0
  );
}

void main() {
  vec2 endpoint = basepoint + iEndpointVector * uLineLength;

  float angle = -atan(iEndpointVector.y, iEndpointVector.x) + PI / 2.0;
  float c = cos(angle);
  float s = sin(angle);
  mat4 rotationMatrix = mat4(
    c,   -s,  0.0, 0.0,
    s,   c,   0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0
  );

  float pointSize = uLineWidth * iLineWidth;
  mat4 modelMatrix = mat4(
    0.5 * pointSize, 0.0,                            0.0, 0.0,
    0.0,             uOrientation * 0.5 * pointSize, 0.0, 0.0,
    0.0,             0.0,                            1.0, 0.0,
    0.0,             0.0,                            0.0, 1.0
  );

  gl_Position = uProjection * uView * translate(endpoint) * rotationMatrix * modelMatrix * vec4(vertex, 0.0, 1.0);

  if (uOrientation > 0.0) {
    vColor = vec4(iColor.rgb, iEndpointOpacity);
  } else {
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
    vec3 premultipliedLineColor = iColor.rgb * iLineOpacity;
    vColor = vec4(iColor.rgb * iEndpointOpacity - premultipliedLineColor, 1.0);
  }
}
