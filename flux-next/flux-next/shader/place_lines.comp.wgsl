// uniform mediump vec4 uColorWheel[6];

// vec4 getColor(vec4 wheel[6], float angle) {
//   float slice = TAU / 6.0;
//   float rawIndex = mod(angle, TAU) / slice;
//   float index = floor(rawIndex);
//   float nextIndex = mod(index + 1.0, 6.0);
//   float interpolate = fract(rawIndex);

//   vec4 currentColor = wheel[int(index)];
//   vec4 nextColor = wheel[int(nextIndex)];
//   return mix(currentColor, nextColor, interpolate);
// }

const tau = 6.283185307179586;

struct Basepoint {
  position: vec2<f32>,
}

// TODO: f16?
struct Line {
  endpoint: vec2<f32>,
  velocity: vec2<f32>,
  color: vec4<f32>,
  color_velocity: vec3<f32>,
  width: f32,
}

struct LineUniforms {
  aspect: f32,
  zoom: f32,
  line_width: f32,
  line_length: f32,
  line_begin_offset: f32,
  line_variance: f32,
  line_noise_scale: vec2<f32>,
  line_noise_offset_1: f32,
  line_noise_offset_2: f32,
  line_noise_blend_factor: f32,
  color_mode: u32,
  delta_time: f32,
}

@group(0) @binding(0) var<uniform> uniforms: LineUniforms;
@group(0) @binding(1) var<storage, read> basepoints: array<Basepoint>;
@group(0) @binding(2) var linear_sampler: sampler;

@group(1) @binding(0) var<storage, read> lines: array<Line>;
@group(1) @binding(1) var<storage, read_write> out_lines: array<Line>;

@group(2) @binding(0) var color_texture: texture_2d<f32>;

@group(3) @binding(0) var velocity_texture: texture_2d<f32>;

fn permute(x: vec4<f32>) -> vec4<f32> {
  return (((x * 34.0) + 1.0) * x) % 289.0;
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
  i = i % 289.0; // Avoid truncation effects in permutation
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

@compute
@workgroup_size(64)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
  let total = arrayLength(&lines);
  let index = global_id.x;
  if (index >= total) {
    return;
  }

  let basepoint = basepoints[index].position;
  let line = lines[index];
  let velocity = textureSampleLevel(velocity_texture, linear_sampler, basepoint, 0.0).xy;
  let noise = snoise(vec3(uniforms.line_noise_scale * basepoint, uniforms.line_noise_offset_1));

  let variance = mix(1.0 - uniforms.line_variance, 1.0, 0.5 + 0.5 * noise);
  let velocity_delta_boost = mix(3.0, 25.0, 1.0 - variance);
  let momentum_boost = mix(3.0, 5.0, variance);

  let new_velocity = (1.0 - uniforms.delta_time * momentum_boost) * line.velocity
    + (uniforms.line_length * velocity - line.endpoint) * velocity_delta_boost * uniforms.delta_time;

  let new_endpoint = line.endpoint + uniforms.delta_time * new_velocity;

  // Basically, smoothstep(0.0, 0.4, length(velocity));
  // Maybe width and opacity should be on different easings.
  let width_boost = saturate(2.5 * length(velocity));
  let new_line_width = width_boost * width_boost * (3.0 - width_boost * 2.0);

  var color: vec3<f32>;
  var color_momentum_boost = 3.0;
  var color_delta_boost = 90.0;

  switch uniforms.color_mode {
    // Original
    case 0u, default: {
      color = vec3<f32>(saturate(vec2<f32>(1.0, 0.66) * (0.5 + velocity)), 0.5);
    }

    // Preset with color wheel
    case 1u: {
      let angle = atan2(velocity.x, velocity.y);
      color = vec3(0.0); // TODO
      // color = getColor(uColorWheel, angle).rgb;
    }

    case 2u: {
      color = textureSampleLevel(color_texture, linear_sampler, 2.0 * velocity + 0.5, 0.0).rgb;
      color_momentum_boost = 5.0;
      color_delta_boost = 10.0;
    }
  }

  let new_color_velocity
    = line.color_velocity * (1.0 - color_momentum_boost * uniforms.delta_time)
    + (color.rgb - line.color.rgb) * color_delta_boost * uniforms.delta_time;

  let new_color = vec4(
    saturate(line.color.rgb + uniforms.delta_time * new_color_velocity),
    width_boost,
  );

  out_lines[index] = Line(
    new_endpoint,
    new_velocity,
    new_color,
    new_color_velocity,
    new_line_width
  );
}
