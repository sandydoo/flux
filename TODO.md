# TODO

### Performance

- [] Move to wgpu in the long-term. WebGL/WebGPU support is very flaky at the
  moment, so I’d like to wait.

- [] Clean up textures/framebuffers, i.e. implement the Drop trait.

### Colors

- [] Does WebGL support DCI-P3? The answer seems to be no (October 2021).

- [] How do we do colors?

  Use the current angle, map colors to polar coordinates, and periodically
  rotate the color wheel?

  Or use a texture (color field) that is advected through the velocity field
  (meh)?

- [] How do we get correct blending when using a second pass for the endpoints?
  Use a renderbuffer and compose? How expensive is that?

### UI

- [] Add configuration options. Use Elm?

### Tools

- [] Try esbuild?
