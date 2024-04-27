#ifdef GL_ES
precision highp float;
precision highp sampler2D;
#endif

layout(std140) uniform FluidUniforms
{
  highp float deltaT;
  highp float dissipation;
  highp vec2 uTexelSize;
};

uniform sampler2D velocityTexture;
uniform float amount;

in vec2 texturePosition;
out vec2 outVelocity;

void main() {
  vec2 size = vec2(textureSize(velocityTexture, 0));
  vec2 texelPosition = floor(size * texturePosition);
  vec2 velocity = texelFetch(velocityTexture, ivec2(texelPosition), 0).xy;
  // Note, that, by multiplying by dx, we’ve “incorrectly” scaled our coordinate system.
  // This is actually a key component of the slow, wriggly “coral reef” look.
  vec2 advectedPosition = ((texelPosition + 0.5) - amount * velocity) / size;
  float decay = 1.0 + dissipation * amount;
  outVelocity = texture(velocityTexture, advectedPosition).xy / decay;
}
