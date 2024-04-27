#ifdef GL_ES
precision mediump float;
precision highp sampler2D;
#endif

layout(std140) uniform FluidUniforms
{
  highp float deltaT;
  highp float dissipation;
  highp vec2 uTexelSize;
};

uniform sampler2D velocityTexture;
uniform sampler2D pressureTexture;

in highp vec2 texturePosition;
out vec2 newVelocity;

void main() {
  float L = textureOffset(pressureTexture, texturePosition, ivec2(-1, 0)).x;
  float R = textureOffset(pressureTexture, texturePosition, ivec2(1, 0)).x;
  float T = textureOffset(pressureTexture, texturePosition, ivec2(0, 1)).x;
  float B = textureOffset(pressureTexture, texturePosition, ivec2(0, -1)).x;

  // Enforce the following boundary conditions:
  //
  //  1. No-slip condition — velocity equals zero at the boundaries.
  //
  //  2. Pure Neumann pressure condition — dp/dn = 0, that is the rate of change
  //     of pressure in the direction normal to the boundary is zero.
  //
  //  GPU Gems has a short section deriving these conditions, but this
  //  implementation is slightly different.
  //
  //  Here, we’re assuming the boundary is the outer edge of the texture grid.
  //
  //  For condition 1, we just set the velocity to zero.
  //
  //  For condition 2, we don’t have to do anything. With texture clamping, any
  //  pressure reads outside the boundary will be set to the last value at the
  //  boundary; so the rate of change across the boundary becomes zero.
  //
  //  I haven’t tested this with an ink/particle texture, so there’s a chance
  //  this doesn’t actually look any good. But it is stable! I’m also unsure of
  //  how the staggered grid affects all of this.
  //
  //  A number of things actually work here: -1.0 adjustment for velocity,
  //  setting just the relevant component of velocity to zero, and flipping
  //  pressures along relevant axis. All seem stable, but experiment!

  vec2 size = vec2(textureSize(velocityTexture, 0));
  ivec2 texelPosition = ivec2(floor(size * texturePosition));
  vec2 velocity = texelFetch(velocityTexture, texelPosition, 0).xy;

  vec2 boundaryCondition = vec2(1.0);
  if (texturePosition.x < uTexelSize.x) {
    boundaryCondition.x = 0.0;
  }
  if (texturePosition.x > 1.0 - uTexelSize.x) {
    boundaryCondition.x = 0.0;
  }
  if (texturePosition.y < uTexelSize.y) {
    boundaryCondition.y = 0.0;
  }
  if (texturePosition.y > 1.0 - uTexelSize.y) {
    boundaryCondition.y = 0.0;
  }

  newVelocity = boundaryCondition * (velocity - 0.5 * vec2(R - L, T - B));
}
