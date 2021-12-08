# TODO

### In progress

#### Notes
Increasing the grid size or the timestep produces oscillations – a wobbly, jello-like effect. Not zeroing out pressure between passes also adds to this effect. Could this be the source of the extra movement in the original? Or is it just extra random acceleration added to each line? Seems a bit too weird. The original is wafty, not wobbly.

- [ ] What if you advect the end points?
- [ ] What state should the line hold? Endpoint? Velocity? Width and length?
- [ ] What if you apply the velocity to the end, taking into account the length of the line? Like a torque? What would happen?
- [x] What happens if you use the velocity at the endpoint of the line? Answer: you get crazy swirlies, like when advecting a color texture.

### General

- [x] Add support for non-square aspect ratio

- [ ] Add antialiasing. The built-in antialiasing is “optional”, apparently.
  Could do MSAA with multisampling renderbuffers? Or FXAA?

### Accessibility (not the 11-ty kind)

- [ ] Add hosted option with CI builds

- [ ] Move to wgpu in the long-term. WebGL/WebGPU support is very flaky at the
  moment, so I’d like to wait.

### Performance

- [ ] Clean up textures/framebuffers, i.e. implement the Drop trait.

- [ ] Use uniform buffers

### Colors

- [ ] Does WebGL support DCI-P3? The answer seems to be no (October 2021).

- [ ] How do we do colors?

  Use the current angle, map colors to polar coordinates, and periodically
  rotate the color wheel?

  Or use a texture (color field) that is advected through the velocity field
  (meh)?

- [ ] How do we get correct blending when using a second pass for the endpoints?
  Use a renderbuffer and compose? How expensive is that?

### UI

- [ ] Add configuration options. Use Elm?

### Tools

- [ ] Try esbuild?
