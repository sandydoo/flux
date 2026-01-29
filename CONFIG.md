# Flux Configuration Guide

The configuration for Flux is stored in `~/.config/flux/settings.json`. This file allows you to customize the simulation physics, the appearance of the lines, and the color schemes.

## Settings Overview

### General
- **`mode`**: The rendering mode.
  - Options: `"Normal"`, `"DebugNoise"`, `"DebugFluid"`, `"DebugPressure"`, `"DebugDivergence"`.
- **`seed`**: (Optional) A string to seed the random number generator for reproducible patterns.

### Colors
- **`colorMode`**: Defines how the lines are colored.
  - **Preset**: Use a built-in color scheme.
    - Options: `"Original"`, `"Plasma"`, `"Poolside"`, `"Freedom"`.
    - Example: `"colorMode": { "Preset": "Plasma" }`
  - **ImageFile**: Sample colors from an image file.
    - Example: `"colorMode": { "ImageFile": "/path/to/image.jpg" }`

### Physics (Fluid Simulation)
- **`fluidSize`**: The resolution of the fluid grid (default: `128`). Higher values look smoother but are more demanding.
- **`viscosity`**: How "thick" the fluid feels (default: `5.0`).
- **`velocityDissipation`**: How quickly the movement slows down over time (default: `0.0`).
- **`fluidFrameRate`**: The target simulation speed (default: `60.0`).
- **`pressureMode`**: How fluid pressure is handled.
  - Example: `{ "ClearWith": 0.0 }`

### Lines (The "Drift" effect)
- **`lineLength`**: The maximum length of the lines (default: `450.0`).
- **`lineWidth`**: The thickness of the lines (default: `9.0`).
- **`lineBeginOffset`**: Adjusts where the lines start appearing (default: `0.4`).
- **`lineVariance`**: Adds randomness to the line behavior (default: `0.55`).
- **`gridSpacing`**: The density of the line grid (default: `15`).
- **`viewScale`**: The overall zoom level of the simulation (default: `1.6`).

### Noise
- **`noiseMultiplier`**: The overall intensity of the flow fields (default: `0.45`).
- **`noiseChannels`**: An array of noise layers that drive the fluid motion. Each layer has:
  - `scale`: The size of the noise pattern.
  - `multiplier`: The strength of this layer.
  - `offsetIncrement`: How fast this noise layer evolves over time.

## Example Configuration

```json
{
  "mode": "Normal",
  "fluidSize": 128,
  "viscosity": 2.0,
  "colorMode": {
    "Preset": "Poolside"
  },
  "lineLength": 600.0,
  "lineWidth": 5.0,
  "viewScale": 2.0
}
```
