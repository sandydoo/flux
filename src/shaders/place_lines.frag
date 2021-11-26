#version 300 es
precision highp float;
precision highp sampler2D;

uniform float deltaT;
uniform mat4 uProjection;
uniform sampler2D basepointTexture;
uniform sampler2D lineStateTexture;
uniform sampler2D velocityTexture;

in vec2 textureCoord;
out vec4 fragColor;

highp float rand(vec2 co) {
  highp float a = 12.9898;
  highp float b = 78.233;
  highp float c = 43758.5453;
  highp float dt= dot(co.xy ,vec2(a,b));
  highp float sn= mod(dt,3.14);
  return fract(sin(sn) * c);
}

void main() {
  vec4 lineState = texture(lineStateTexture, textureCoord);
  vec2 position = lineState.rg;
  vec2 velocity = lineState.ba;

  vec2 velocityAtPosition = texture(velocityTexture, (uProjection * vec4(position, 0.0, 1.0)).xy * 0.5 + 0.5).xy;
  vec2 deltaVelocity = velocityAtPosition - velocity;

  velocity += clamp(rand(position), 0.7, 1.0) * deltaVelocity * deltaT;

  fragColor = vec4(position, velocity);
}
