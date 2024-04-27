#ifdef GL_ES
precision mediump float;
#endif

// Defined at compile-time
// #define CHANNEL_COUNT 3

struct Channel {
  mediump vec2 uScale;
  mediump float uOffset1;
  mediump float uOffset2;
  mediump float uBlendFactor;
  mediump float uMultiplier;
};

layout(std140) uniform Channels
{
  Channel uChannels[CHANNEL_COUNT];
};

in vec2 clipPosition;
in vec2 texturePosition;
out vec2 noise;

vec3 mod289(vec3 x) {
  return x - floor(x * (1.0 / 289.0)) * 289.0;
}

vec4 mod289(vec4 x) {
  return x - floor(x * (1.0 / 289.0)) * 289.0;
}

vec4 permute(vec4 x) {
  return mod289(((x * 34.0) + 1.0) * x);
}

vec4 taylorInvSqrt(vec4 r) {
  return 1.79284291400159 - 0.85373472095314 * r;
}

float snoise(vec3 v) {
  const vec2 C = vec2(1.0 / 6.0, 1.0 / 3.0);

  // First corner
  vec3 i = floor(v + dot(v, C.yyy));
  vec3 x0 = v - i + dot(i, C.xxx);

  // Other corners
  vec3 g = step(x0.yzx, x0.xyz);
  vec3 l = 1.0 - g;
  vec3 i1 = min(g.xyz, l.zxy);
  vec3 i2 = max(g.xyz, l.zxy);

  // x1 = x0 - i1  + 1.0 * C.xxx;
  // x2 = x0 - i2  + 2.0 * C.xxx;
  // x3 = x0 - 1.0 + 3.0 * C.xxx;
  vec3 x1 = x0 - i1 + C.xxx;
  vec3 x2 = x0 - i2 + C.yyy;
  vec3 x3 = x0 - 0.5;

  // Permutations
  i = mod289(i); // Avoid truncation effects in permutation
  vec4 p =
    permute(permute(permute(i.z + vec4(0.0, i1.z, i2.z, 1.0))
                          + i.y + vec4(0.0, i1.y, i2.y, 1.0))
                          + i.x + vec4(0.0, i1.x, i2.x, 1.0));

  // Gradients: 7x7 points over a square, mapped onto an octahedron.
  // The ring size 17*17 = 289 is close to a multiple of 49 (49*6 = 294)
  vec4 j = p - 49.0 * floor(p * (1.0 / 49.0)); // mod(p,7*7)

  vec4 x_ = floor(j * (1.0 / 7.0));
  vec4 y_ = floor(j - 7.0 * x_ ); // mod(j,N)

  vec4 x = x_ * (2.0 / 7.0) + 0.5 / 7.0 - 1.0;
  vec4 y = y_ * (2.0 / 7.0) + 0.5 / 7.0 - 1.0;

  vec4 h = 1.0 - abs(x) - abs(y);

  vec4 b0 = vec4(x.xy, y.xy);
  vec4 b1 = vec4(x.zw, y.zw);

  //vec4 s0 = vec4(lessThan(b0, 0.0)) * 2.0 - 1.0;
  //vec4 s1 = vec4(lessThan(b1, 0.0)) * 2.0 - 1.0;
  vec4 s0 = floor(b0) * 2.0 + 1.0;
  vec4 s1 = floor(b1) * 2.0 + 1.0;
  vec4 sh = -step(h, vec4(0.0));

  vec4 a0 = b0.xzyw + s0.xzyw * sh.xxyy;
  vec4 a1 = b1.xzyw + s1.xzyw * sh.zzww;

  vec3 g0 = vec3(a0.xy, h.x);
  vec3 g1 = vec3(a0.zw, h.y);
  vec3 g2 = vec3(a1.xy, h.z);
  vec3 g3 = vec3(a1.zw, h.w);

  // Normalise gradients
  vec4 norm = taylorInvSqrt(vec4(dot(g0, g0), dot(g1, g1), dot(g2, g2), dot(g3, g3)));
  g0 *= norm.x;
  g1 *= norm.y;
  g2 *= norm.z;
  g3 *= norm.w;

  // Mix final noise value
  vec4 m = max(0.6 - vec4(dot(x0, x0), dot(x1, x1), dot(x2, x2), dot(x3, x3)), 0.0);
  m = m * m;
  m = m * m;

  vec4 px = vec4(dot(x0, g0), dot(x1, g1), dot(x2, g2), dot(x3, g3));
  return 42.0 * dot(m, px);
}

vec2 makeNoisePair(vec3 params) {
  return vec2(snoise(params), snoise(params + vec3(8.0, -8.0, 0.0)));
}

vec2 makeNoise(Channel channel) {
  vec2 scale = channel.uScale * texturePosition;
  vec2 noise1 = makeNoisePair(vec3(scale, channel.uOffset1));
  vec2 noise = noise1;

  if (channel.uBlendFactor > 0.0) {
    vec2 noise2 = makeNoisePair(vec3(scale, channel.uOffset2));
    noise = mix(noise1, noise2, channel.uBlendFactor);
  }

  return channel.uMultiplier * noise;
}

void main() {
  noise = vec2(0.0);

  // Safari and OpenGL on macOS aren’t happy with “dynamic indexing” inside a
  // for-loop, so we unwrap the loop.
  #if CHANNEL_COUNT > 0
    noise += makeNoise(uChannels[0]);
  #endif
  #if CHANNEL_COUNT > 1
    noise += makeNoise(uChannels[1]);
  #endif
  #if CHANNEL_COUNT > 2
    noise += makeNoise(uChannels[2]);
  #endif
  #if CHANNEL_COUNT > 3
    noise += makeNoise(uChannels[3]);
  #endif

  // TODO: make this configurable
  noise *= 0.45;
}

