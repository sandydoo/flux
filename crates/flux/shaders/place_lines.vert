#define PI 3.1415926535897932384626433832795

precision highp float;
precision highp sampler2D;

// static input
in vec2 basepoint;

// dynamic input
in vec2 iEndpointVector;
in vec2 iVelocityVector;
in vec4 iColor;
in float iLineWidth;
in float iLineOpacity;
in float iEndpointOpacity;

uniform float deltaT;
// uniform float elapsedTime;
uniform float uBlendProgress;
uniform float uSpringStiffness;
uniform float uSpringVariance;
uniform float uSpringMass;
uniform float uSpringDamping;
uniform float uSpringRestLength;
uniform float uLineFadeOutLength;
uniform float uMaxLineVelocity;
uniform float uAdjustAdvection;
uniform float uAdvectionDirection;
uniform mediump vec4 uColorWheel[6];
uniform mat4 uProjection;

uniform sampler2D velocityTexture;
uniform sampler2D noiseTexture;

// transform feedback output
out vec2 vEndpointVector;
out vec2 vVelocityVector;
out vec4 vColor;
out float vLineWidth;
out float vLineOpacity;
out float vEndpointOpacity;

vec2 safeNormalize(vec2 v) {
  if (length(v) == 0.0) {
    return vec2(0.0);
  }
  return normalize(v);
}

float clampTo(float value, float max) {
  float current = value + 0.0001;
  return min(current, max) / current;
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

float springForce(float stiffness, float displacement,  float damping, float velocity, float mass) {
  return ((-stiffness * displacement) + (-damping * velocity)) / mass;
}

float random1f(in vec2 st) {
  return fract(sin(dot(st.xy, vec2(12.9898, 78.233))) * 43758.5453123);
}

float easeInCirc(float x) {
  return 1.0 - sqrt(1.0 - pow(x, 2.0));
}

float easeOutCirc(float x) {
  return sqrt(1.0 - pow(x - 1.0, 2.0));
}

float inverseEaseInCirc(float x) {
  return 1.0 - easeInCirc(x);
}

float endpointCurve(float lineLength, float lineOpacity, float fadeInPoint) {
  return mix(
    0.75 * easeOutCirc(smoothstep(uLineFadeOutLength - 0.01, 1.0, lineLength)),
    lineOpacity,
    smoothstep(fadeInPoint - 0.2, fadeInPoint, lineLength)
  );
}

void main() {
  vec2 endpointDirection = safeNormalize(iEndpointVector);
  float currentLength = length(iEndpointVector);

  // Velocity
  vec2 basepointInClipSpace = 0.5 + 0.5 * (uProjection * vec4(basepoint, 0.0, 1.0)).xy;
  vec2 currentVelocityVector = texture(velocityTexture, basepointInClipSpace).xy;
  vec2 deltaVelocity = currentVelocityVector - iVelocityVector;

  vec2 velocityDirection = normalize(uAdvectionDirection * iVelocityVector);
  vec2 lineDirection = normalize(iEndpointVector);
  float directionAlignment = clamp(dot(lineDirection, velocityDirection), -1.0, 1.0);

  float mass = uSpringMass * (1.0 + uSpringVariance * random1f(basepoint));

  float advectionDirection = 1.0;
  vec2 noise = texture(noiseTexture, basepointInClipSpace).xy;
  if (noise.x <= 0.0) {
    // advectionDirection = -1.0;
  }
    vVelocityVector = iVelocityVector + deltaT * (0.0 * currentVelocityVector + 1.0 * deltaVelocity);

    // Spring forces
    float springbackForce = springForce(
      uSpringStiffness,
      currentLength - uSpringRestLength,
      uSpringDamping,
      directionAlignment * length(vVelocityVector),
      mass
    );
    vVelocityVector += uAdvectionDirection * endpointDirection * springbackForce * deltaT;
  // } else {
    // advectionDirection = -1.0;
    // vVelocityVector = iVelocityVector;
  // }

  // Jiggle stuff
  // vec2 noise = texture(noiseTexture, basepointInClipSpace).xy;
  // float frequency = 10.0;
  // float sx = 0.006 * snoise(vec3(basepointInClipSpace * frequency, elapsedTime));
  // float sy = 0.006 * snoise(vec3(basepointInClipSpace * frequency, 2.0 + elapsedTime));
  // length(force) > uBlendThreshold &&
  // if (uBlendProgress < 1.0 && length(noise) > 0.2) {
    // vVelocityVector += 0.002 * uBlendProgress * noise;
    // vVelocityVector *= 1.0 - 0.1 * noise.x;
  // }
  // vec2 adjustAdvection = uAdjustAdvection * (1.0 + 1.0 * noise.xy);
  // float adjustAdvection = uAdjustAdvection * length(noise.xy);

  // Cap line velocity
  vVelocityVector *= clampTo(length(vVelocityVector), uMaxLineVelocity);

  // Advect forward
  vEndpointVector = iEndpointVector + uAdjustAdvection * advectionDirection * vVelocityVector * deltaT;
  currentLength = length(vEndpointVector);

  // Color
  float angle = mod(
    PI / 4.0 * currentLength + (PI + atan(iEndpointVector.y, iEndpointVector.x)),
    2.0 * PI
  );
  vec4 newColor = vec4(getColor(uColorWheel, angle), 0.0);
  vec4 colorDiff = newColor - iColor;
  vColor = clamp(
    iColor + colorDiff * deltaT,
    0.0,
    1.0
  );
  // Debug spring extension
  // vColor = mix(vColor, vec4(1.0), smoothstep(0.95, 1.05, currentLength));

  // Width
  float clampedLength = clamp(currentLength, 0.0, 1.0);
  float lineWidthBoost = 1.6;
  vLineWidth = clamp(
    iLineWidth + inverseEaseInCirc(clampedLength) * lineWidthBoost * uAdjustAdvection * directionAlignment * length(vVelocityVector) * deltaT,
    0.05,
    1.0
  );

  // Opacity
  vLineOpacity = smoothstep(uLineFadeOutLength, 1.0, currentLength);
  vEndpointOpacity = endpointCurve(currentLength, iLineOpacity, 0.8);
}
