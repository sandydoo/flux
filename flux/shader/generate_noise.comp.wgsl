struct NoiseUniforms {
  multiplier: f32,
}

struct Channel {
  scale: vec2<f32>,
  offset_1: f32,
  offset_2: f32,
  blend_factor: f32,
  multiplier: f32,
  padding: vec2<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: NoiseUniforms;
@group(0) @binding(1) var<storage, read> channels: array<Channel>;
@group(0) @binding(2) var out_texture: texture_storage_2d<rg32float, write>;

fn mod289(x: vec4<f32>) -> vec4<f32> {
  return x - floor(x * (1.0 / 289.0)) * 289.0;
}

fn permute(x: vec4<f32>) -> vec4<f32> {
  return mod289(((x * 34.0) + 1.0) * x);
}

fn snoise(v: vec3<f32>) -> f32 {
  let C = vec2(1.0 / 6.0, 1.0 / 3.0);

  // First corner
  var i = floor(v + dot(v, C.yyy));
  let x0 = v - i + dot(i, C.xxx);

  // Other corners
  let g = step(x0.yzx, x0.xyz);
  let l = 1.0 - g;
  let i1 = min(g.xyz, l.zxy);
  let i2 = max(g.xyz, l.zxy);

  // x1 = x0 - i1  + 1.0 * C.xxx;
  // x2 = x0 - i2  + 2.0 * C.xxx;
  // x3 = x0 - 1.0 + 3.0 * C.xxx;
  let x1 = x0 - i1 + C.xxx;
  let x2 = x0 - i2 + C.yyy;
  let x3 = x0 - 0.5;

  // Permutations
  i = mod289(vec4<f32>(i.x, i.y, i.z, 0.0)).xyz; // Avoid truncation effects in permutation
  let p =
    permute(permute(permute(i.z + vec4(0.0, i1.z, i2.z, 1.0))
                          + i.y + vec4(0.0, i1.y, i2.y, 1.0))
                          + i.x + vec4(0.0, i1.x, i2.x, 1.0));

  // Gradients: 7x7 points over a square, mapped onto an octahedron.
  // The ring size 17*17 = 289 is close to a multiple of 49 (49*6 = 294)
  let j = p - 49.0 * floor(p * (1.0 / 49.0)); // mod(p,7*7)

  let x_ = floor(j * (1.0 / 7.0));
  let y_ = floor(j - 7.0 * x_); // mod(j,N)

  let x = x_ * (2.0 / 7.0) + 0.5 / 7.0 - 1.0;
  let y = y_ * (2.0 / 7.0) + 0.5 / 7.0 - 1.0;

  let h = 1.0 - abs(x) - abs(y);

  let b0 = vec4(x.xy, y.xy);
  let b1 = vec4(x.zw, y.zw);

  //vec4 s0 = vec4(lessThan(b0, 0.0)) * 2.0 - 1.0;
  //vec4 s1 = vec4(lessThan(b1, 0.0)) * 2.0 - 1.0;
  let s0 = floor(b0) * 2.0 + 1.0;
  let s1 = floor(b1) * 2.0 + 1.0;
  let sh = -step(h, vec4(0.0));

  let a0 = b0.xzyw + s0.xzyw * sh.xxyy;
  let a1 = b1.xzyw + s1.xzyw * sh.zzww;

  var g0 = vec3(a0.xy, h.x);
  var g1 = vec3(a0.zw, h.y);
  var g2 = vec3(a1.xy, h.z);
  var g3 = vec3(a1.zw, h.w);

  // Normalise gradients
  let norm = inverseSqrt(vec4(dot(g0, g0), dot(g1, g1), dot(g2, g2), dot(g3, g3)));
  g0 *= norm.x;
  g1 *= norm.y;
  g2 *= norm.z;
  g3 *= norm.w;

  // Mix final noise value
  var m = max(0.6 - vec4(dot(x0, x0), dot(x1, x1), dot(x2, x2), dot(x3, x3)), vec4(0.0));
  m = m * m;
  m = m * m;

  let px = vec4(dot(x0, g0), dot(x1, g1), dot(x2, g2), dot(x3, g3));
  return 42.0 * dot(m, px);
}

fn make_noise_pair(params: vec3<f32>) -> vec2<f32> {
  return vec2(snoise(params), snoise(params + vec3(8.0, -8.0, 0.0)));
}

fn make_noise(texel_position: vec2<f32>, channel: Channel) -> vec2<f32> {
  let scale = channel.scale * texel_position;
  let noise1 = make_noise_pair(vec3(scale, channel.offset_1));
  var noise = noise1;

  if (channel.blend_factor > 0.0) {
    let noise2 = make_noise_pair(vec3(scale, channel.offset_2));
    noise = mix(noise1, noise2, channel.blend_factor);
  }

  return channel.multiplier * noise;
}

@compute
@workgroup_size(16, 16, 1)
fn main(
  @builtin(global_invocation_id) global_id: vec3<u32>,
) {
  let size = vec2<f32>(textureDimensions(out_texture));
  let texel_position = (vec2<f32>(global_id.xy) + 0.5) / size;

  var noise = vec2f(0.0);
  let numChannels = arrayLength(&channels);
  var i : u32 = 0u;
  loop {
    if (i >= numChannels) {
      break;
    }

    let channel = channels[i];
    noise += make_noise(texel_position, channel);

    continuing {
      i = i + 1u;
    }
  }

  // TODO: make strength factor configurable
  textureStore(out_texture, global_id.xy, vec4<f32>(noise * uniforms.multiplier, 0.0, 0.0));
}
