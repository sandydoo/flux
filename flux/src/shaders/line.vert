#version 300 es
precision mediump float;

in vec2 lineVertex;
in vec2 basepoint;


in vec2 iEndpointVector;
in vec2 iVelocityVector;
in float iLineWidth;
in vec4 iColor;
in float iOpacity;

layout(std140) uniform Projection
{
  mat4 uProjection;
  mat4 uView;
};

// layout(std140) uniform LineUniforms
// {
//   mediump float uLineWidth;
//   mediump float uLineLength;
//   mediump float uLineBeginOffset;
//   mediump float uLineBaseOpacity;
// };
layout(std140) uniform LineUniforms
{
  mediump float uLineWidth;
  mediump float uLineLength;
  mediump float uLineBeginOffset;
  mediump float uLineBaseOpacity;
  mediump float uLineFadeOutLength;
  mediump float deltaT;
  mediump vec2 padding;
  mediump vec3 uColorWheel[6];
};

out vec2 vVertex;
out vec3 vColor;
out float vOpacity;

mat4 scale(vec3 v) {
  return mat4(
    v.x, 0.0, 0.0, 0.0,
    0.0, v.y, 0.0, 0.0,
    0.0, 0.0, v.z, 0.0,
    0.0, 0.0, 0.0, 1.0
  );
}

void main() {
  vec2 endpoint = basepoint + iEndpointVector * uLineLength;

  vec2 xBasis = endpoint - basepoint;
  vec2 yBasis = normalize(vec2(-xBasis.y, xBasis.x));
  vec2 point = basepoint + xBasis * lineVertex.x + yBasis * (iLineWidth * uLineWidth) * lineVertex.y;

  gl_Position = uProjection * uView * vec4(point, 0.0, 1.0);

  vVertex = lineVertex;
  vColor = iColor.rgb;
  vOpacity = iOpacity;
}
