# TODO

### General

- Move to wgpu in the long-term. WebGL/WebGPU support is very flaky at the moment, so I’d like to wait.

### Colors

- [ ] Colorspace: Does WebGL support DCI-P3? The answer seems to be no (October 2021).
- [ ] How do we do colors? We want grouping? Use a texture (color field) that is advected through the velocity field?
- [ ] Blending: figure out correct blend mode to lighten the lines when they overlap.
   How do we avoid color bleed from the background? Is this where we can leverage a stencil buffer?
