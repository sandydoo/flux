#version 300 es
#define PI 3.1415926535897932384626433832795

precision highp float;
precision highp sampler2D;

// static input
in vec2 basepoint;
in uint lineIndex;

// dynamic input
in vec2 iEndpointVector;
in vec2 iVelocityVector;
// in vec4 iColor;
in float iLineWidth;
// in float iOpacity;

uniform float deltaT;
uniform mat4 uProjection;
uniform vec3 uColorWheel[6];
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

vec3 getColor(vec3 wheel[6], float angle) {
  float slice = 2.0 * PI / 6.0;
  float rawIndex = angle / slice;
  float index = floor(rawIndex);
  float nextIndex = mod(index + 1.0, 6.0);
  float interpolate = fract(rawIndex);

  vec3 currentColor = wheel[int(index)];
  vec3 nextColor = wheel[int(nextIndex)];
  return mix(currentColor, nextColor, interpolate);
}

void main() {
  vec2 basepointInClipSpace = (uProjection * vec4(basepoint, 0.0, 1.0)).xy;
  vec2 currentVelocityVector = texture(velocityTexture, basepointInClipSpace * 0.5 + 0.5).xy;

  vec2 deltaVelocity = currentVelocityVector - iVelocityVector;

  vVelocityVector = iVelocityVector + deltaVelocity * deltaT;

  float stiffness = 0.4;
  vec2 backpressure = -stiffness * iEndpointVector;
  vVelocityVector -= backpressure * deltaT;

  // advect forward
  vEndpointVector = iEndpointVector - vVelocityVector * deltaT;

  float currentLength = length(vEndpointVector);
  vEndpointVector *= clampTo(currentLength, 1.0);

  vec2 velocityDirection = normalize(-vVelocityVector);
  vec2 lineDirection = normalize(vEndpointVector);
  float directionAlignment = clamp(dot(lineDirection, velocityDirection), -1.0, 1.0);
  float directionMultiplier = 0.5 * length(deltaVelocity);
  float minWidth = max(0.2, 1.0 * currentLength);
  float maxWidth = min(1.0, 5.0 * currentLength);

  float angle = mod(
    PI * currentLength + (PI + atan(iEndpointVector.y, iEndpointVector.x)),
    2.0 * PI
  );
  vColor = vec4(getColor(uColorWheel, angle), 0.0);

  vLineWidth = clamp(
    iLineWidth + directionMultiplier * deltaT * directionAlignment,
    minWidth,
    maxWidth
  );

  vOpacity = smoothstep(0.05, 0.2, currentLength);
}
