#version 300 es
precision highp float;
precision highp sampler2D;

// static input
in vec2 basepoint;
in uint lineIndex;

// dynamic input
in vec2 iEndpointVector;
in vec2 iVelocityVector;
in float iLineWidth;

uniform float deltaT;
uniform mat4 uProjection;
uniform sampler2D velocityTexture;

// transform feedback output
out vec2 vEndpointVector;
out vec2 vVelocityVector;
out float vLineWidth;

float clampTo(float value, float max) {
  return min(value, max) / value;
}

void main() {
  float maxAcceleration = 0.05;
  float maxVelocity = 1.0;

  vec2 basepointInClipSpace = (uProjection * vec4(basepoint, 0.0, 1.0)).xy;
  vec2 currentVelocityVector = texture(velocityTexture, basepointInClipSpace * 0.5 + 0.5).xy;

  vec2 deltaVelocity = currentVelocityVector - iVelocityVector;
  deltaVelocity *= clampTo(length(deltaVelocity), maxAcceleration);

  vVelocityVector = iVelocityVector + deltaVelocity * deltaT;
  // vVelocityVector = currentVelocityVector;

  float stiffness = 0.02;
  vec2 backpressure = -stiffness * iEndpointVector;
  // backpressure *= clampTo(length(backpressure), 0.0);
  vVelocityVector -= backpressure * deltaT;

  // vVelocityVector *= clampTo(length(vVelocityVector), maxVelocity);

  // advect forward
  vEndpointVector = iEndpointVector - vVelocityVector * deltaT;

  float currentLength = length(vEndpointVector);
  vEndpointVector *= clampTo(currentLength, 1.0);

  // TODO: change width based on length AND velocity direction
  vLineWidth = 0.1 + 0.9 * smoothstep(0.1, 1.0, currentLength);
}
