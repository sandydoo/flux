#ifdef GL_ES
precision mediump float;
#endif

in vec2 vVertex;
in vec4 vColor;

layout(std140) uniform LineUniforms
{
  mediump float uLineWidth;
  mediump float uLineLength;
  mediump float uLineBeginOffset;
  mediump float uLineVariance;
  highp float lineNoiseOffset1;
  highp float lineNoiseOffset2;
  highp float lineNoiseBlendFactor;
  highp float deltaTime;
};

out vec4 fragColor;

void main() {
  float fade = smoothstep(uLineBeginOffset, 1.0, vVertex.y);

  float xOffset = abs(vVertex.x);
  float antialiasing = 1.0 - smoothstep(0.5 - fwidth(xOffset), 0.5, xOffset);

  fragColor = vec4(vColor.rgb, vColor.a * fade * antialiasing);
}
