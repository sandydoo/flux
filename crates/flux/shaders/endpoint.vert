#define PI 3.1415926535897932384626433832795
precision highp float;

layout(std140) uniform Projection
{
  mat4 uProjection;
  mat4 uView;
};

layout(std140) uniform LineUniforms
{
  highp float uLineWidth;
  highp float uLineLength;
  highp float uLineBeginOffset;
  highp float uLineFadeOutLength;
};

in vec2 vertex;
in vec2 basepoint;

in vec2 iEndpointVector;
in vec2 iVelocityVector;
in float iLineWidth;
in vec4 iColor;
in float iOpacity;

out vec2 vPosition;
out vec3 vColor;
out vec3 vPremultipliedLineColor;
out float vOpacity;
out vec2 vPerpendicularVector;

mat4 translate(vec3 v) {
  return mat4(
    vec4(1.0, 0.0, 0.0, 0.0),
    vec4(0.0, 1.0, 0.0, 0.0),
    vec4(0.0, 0.0, 1.0, 0.0),
    vec4(v.x, v.y, v.z, 1.0)
  );
}

mat4 rotateZ(float angle) {
  float s = sin(angle);
  float c = cos(angle);

  return mat4(
    c,   -s,  0.0, 0.0,
    s,  c,    0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0
  );
}

// TODO: A lot of this shared with lines. Can we do something about that?
void main() {
  vec2 endpoint = basepoint + iEndpointVector * uLineLength;

  float pointSize = uLineWidth * iLineWidth;
  mat4 modelMatrix = mat4(
    0.5 * pointSize, 0.0, 0.0, 0.0,
    0.0, 0.5 * pointSize, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0
  );

  gl_Position = uProjection * uView * translate(vec3(endpoint, 0.0)) * modelMatrix * vec4(vertex, 0.0, 1.0);

  float endpointOpacity = smoothstep(uLineFadeOutLength, uLineFadeOutLength + 0.3, length(iEndpointVector));
  vPosition = vertex;
  vColor = iColor.rgb;
  vPremultipliedLineColor = vColor * iOpacity;
  vOpacity = endpointOpacity;
  vPerpendicularVector = (rotateZ(PI / 2.0) * vec4(iEndpointVector, 0.0, 1.0)).xy;
}
