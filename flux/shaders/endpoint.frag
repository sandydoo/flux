#ifdef GL_ES
precision mediump float;
#endif

in vec2 vVertex;
in vec4 vColor;
in float endpointOpacity;

out vec4 fragColor;

void main() {
  vec4 color;
  if (vVertex.y >= 0.0) {
    color = vec4(vColor.rgb, endpointOpacity);
  } else {
    // The color of the lower half of the endpoint is less obvious. We’re
    // drawing over part of the line, so to match the color of the upper
    // endpoint, we have to do some math. Luckily, we know the premultiplied
    // color of the line underneath, so we can reverse the blend equation to get
    // the right color.
    //
    // GL_BLEND(SRC_ALPHA, ONE) = srcColor * srcAlpha + dstColor * srcAlpha
    // = vColor * vEndpointOpacity + vColor * vLineOpacity
    //
    // Remember, we’ve already premultiplied our colors! The opacity should be
    // 1.0 to disable more opacity blending!
    vec3 premultipliedLineColor = vColor.rgb * vColor.a;
    color = vec4(vColor.rgb * endpointOpacity - premultipliedLineColor, 1.0);
    // color = vec4(1.0);
  }

  float distance = length(vVertex);
  float antialiasing = 1.0 - smoothstep(1.0 - fwidth(distance), 1.0, distance);
  // if (vVertex.y == 0.0) {
    // antialiasing *= smoothstep(0.0, fwidth(abs(vVertex.y)), abs(vVertex.y));
  // }
  fragColor = vec4(color.rgb, color.a * antialiasing);
}
