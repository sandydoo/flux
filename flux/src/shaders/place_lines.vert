#version 300 es
#define PI 3.1415926535897932384626433832795

precision highp float;
precision highp sampler2D;

// static input
in vec2 basepoint;
// in uint lineIndex;

// dynamic input
in vec2 iEndpointVector;
in vec2 iVelocityVector;
in vec4 iColor;
in float iLineWidth;
in float iOpacity;

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
  highp float uLineBaseOpacity;
  highp float uLineFadeOutLength;
  highp float deltaT;
  mediump vec2 padding;
  mediump vec4 uColorWheel[6];
};

uniform sampler2D velocityTexture;

// transform feedback output
out vec2 vEndpointVector;
out vec2 vVelocityVector;
out vec4 vColor;
out float vLineWidth;
out float vOpacity;


float clampTo(float value, float max) {
  return min(value, max) / value;
}

vec3 getColor(vec4 wheel[6], float angle) {
  float slice = 2.0 * PI / 6.0;
  float rawIndex = angle / slice;
  float index = floor(rawIndex);
  float nextIndex = mod(index + 1.0, 6.0);
  float interpolate = fract(rawIndex);

  vec3 currentColor = wheel[int(index)].rgb;
  vec3 nextColor = wheel[int(nextIndex)].rgb;
  return mix(currentColor, nextColor, interpolate);
}

float springForce(float stiffness, float mass, float displacement) {
  return (-stiffness * displacement) / mass;
}

float random1f(in vec2 st) {
  return fract(sin(dot(st.xy, vec2(12.9898, 78.233))) * 43758.5453123);
}

void main() {
  float springStiffness = 0.12;
  float springRestLength = 0.01;
  float springVariance = 0.12; // 12%
  float mass = 7.0;

  // Velocity
  vec2 basepointInClipSpace = (uProjection * vec4(basepoint, 0.0, 1.0)).xy;
  vec2 currentVelocityVector = texture(velocityTexture, basepointInClipSpace * 0.5 + 0.5).xy;
  vec2 deltaVelocity = currentVelocityVector - iVelocityVector;
  vVelocityVector = iVelocityVector + (deltaVelocity / mass) * deltaT;

  // Spring forces
  float variance = 1.0 + springVariance * (random1f(basepoint ) * 2.0 - 1.0);
  float currentLength = length(iEndpointVector);
  vec2 direction;
  if (currentLength == 0.0) {
    direction = vec2(0.0);
  } else {
    direction = normalize(iEndpointVector);
  }

  // Main spring
  vVelocityVector -= springForce(
    variance * springStiffness,
    mass, // mass
    currentLength - springRestLength
  ) * direction * deltaT;

  // Second spring after full extension
  if (currentLength > 1.0) {
    vVelocityVector -= springForce(
      variance * 0.1,
      mass, // mass
      currentLength - 1.0
    ) * direction * deltaT;
  }

  // Advect forward
  vEndpointVector = iEndpointVector - vVelocityVector * deltaT;
  currentLength = length(vEndpointVector);

  // Color
  float angle = mod(
    PI * currentLength + (PI + atan(iEndpointVector.y, iEndpointVector.x)),
    2.0 * PI
  );
  vColor = vec4(getColor(uColorWheel, angle), 0.0);

  // Width
  vec2 velocityDirection = normalize(-vVelocityVector);
  vec2 lineDirection = normalize(vEndpointVector);
  float directionAlignment = clamp(dot(lineDirection, velocityDirection), -1.0, 1.0);
  float directionMultiplier = 1.0 * length(deltaVelocity);
  float referenceWidth = smoothstep(0.00, 0.6, currentLength);

  vLineWidth = clamp(
    iLineWidth + directionMultiplier * deltaT * directionAlignment,
    max(0.0, referenceWidth * 0.7),
    min(1.0, referenceWidth * 2.0)
  );

  // Opacity
  vOpacity = smoothstep(uLineFadeOutLength, uLineFadeOutLength + 0.2, currentLength);
}
