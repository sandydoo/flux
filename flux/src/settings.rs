use serde::{Deserialize, Serialize};

/// Conversion from persisted line units to logical pixels. UI normalization
/// leaves the serialized values and their rendered size unchanged.
pub const LOGICAL_PIXELS_PER_LINE_UNIT: f32 = 20.0 / 49.0;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub mode: Mode,
    pub seed: Option<String>,

    /// Base fluid texture side on a 16:10 surface. Longer spans extend one axis,
    /// bounded by the GPU budget; logical scene size is independent of quality.
    pub fluid_size: u32,
    pub fluid_frame_rate: f32,
    pub fluid_timestep: f32,
    pub viscosity: f32,
    pub velocity_dissipation: f32,
    pub pressure_mode: PressureMode,
    pub diffusion_iterations: u32,
    pub pressure_iterations: u32,

    pub color_mode: ColorMode,

    /// Nominal length in legacy line units (20/49 logical pixels), before view scale.
    /// Rendered length also depends on the simulated endpoint magnitude.
    pub line_length: f32,
    /// Maximum width in the same units as line_length, before view scale.
    pub line_width: f32,
    pub line_begin_offset: f32,
    pub line_variance: f32,
    /// Logical pixels between basepoints, before view scale.
    pub grid_spacing: u32,
    /// Scene zoom: scales line dimensions and basepoint spacing and crops the field.
    pub view_scale: f32,
    /// User size multiplier, independent of OS display scaling and legacy zoom.
    /// Missing in older presets means 100%, preserving their existing size.
    pub overall_scale: f32,

    pub noise_multiplier: f32,
    pub noise_channels: Vec<Noise>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            mode: Mode::Normal,
            seed: None,
            fluid_size: 128,
            fluid_frame_rate: 60.0,
            fluid_timestep: 1.0 / 60.0,
            viscosity: 5.0,
            velocity_dissipation: 0.0,
            pressure_mode: PressureMode::ClearWith(0.0),
            diffusion_iterations: 3,
            pressure_iterations: 20,
            color_mode: ColorMode::Preset(ColorPreset::Original),
            line_length: 450.0,
            line_width: 9.0,
            line_begin_offset: 0.4,
            line_variance: 0.55,
            grid_spacing: 15,
            view_scale: 1.6,
            overall_scale: 1.0,
            noise_multiplier: 0.45,
            noise_channels: vec![
                Noise {
                    scale: 2.8,
                    multiplier: 1.0,
                    offset_increment: 0.001,
                },
                Noise {
                    scale: 15.0,
                    multiplier: 0.7,
                    offset_increment: 0.001 * 6.0,
                },
                Noise {
                    scale: 30.0,
                    multiplier: 0.5,
                    offset_increment: 0.001 * 12.0,
                },
            ],
        }
    }
}

impl Settings {
    pub fn overall_scale(&self) -> f32 {
        if self.overall_scale.is_finite() && self.overall_scale > 0.0 {
            self.overall_scale.clamp(0.1, 10.0)
        } else {
            1.0
        }
    }

    /// Nominal logical length; actual length follows the simulated endpoint.
    pub fn line_length_pixels(&self) -> f32 {
        self.line_length * LOGICAL_PIXELS_PER_LINE_UNIT * self.view_scale * self.overall_scale()
    }

    pub fn line_width_pixels(&self) -> f32 {
        self.line_width * LOGICAL_PIXELS_PER_LINE_UNIT * self.view_scale * self.overall_scale()
    }
}

#[derive(Clone, Default, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Normal,
    DebugNoise,
    DebugFluid,
    DebugPressure,
    DebugDivergence,
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum PressureMode {
    Retain,
    ClearWith(f32),
}

impl Default for PressureMode {
    fn default() -> Self {
        Self::ClearWith(0.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum ColorMode {
    Preset(ColorPreset),
    ImageFile(std::path::PathBuf),
}

impl Default for ColorMode {
    fn default() -> Self {
        Self::Preset(Default::default())
    }
}

impl From<ColorMode> for u32 {
    fn from(val: ColorMode) -> Self {
        match val {
            ColorMode::Preset(ColorPreset::Original) => 0,
            ColorMode::Preset(_) => 1,
            ColorMode::ImageFile(_) => 2,
        }
    }
}

#[derive(Copy, Clone, Default, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum ColorPreset {
    #[default]
    Original,
    Plasma,
    Poolside,
    Freedom,
}

impl ColorPreset {
    pub fn to_color_wheel(&self) -> Option<[f32; 24]> {
        match self {
            ColorPreset::Plasma => Some(COLOR_SCHEME_PLASMA),
            ColorPreset::Poolside => Some(COLOR_SCHEME_POOLSIDE),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Noise {
    pub scale: f32,
    pub multiplier: f32,
    pub offset_increment: f32,
}

#[rustfmt::skip]
pub static COLOR_SCHEME_PLASMA: [f32; 24] = [
    60.219  / 255.0, 37.2487 / 255.0, 66.4301 / 255.0, 1.0,
    170.962 / 255.0, 54.4873 / 255.0, 50.9661 / 255.0, 1.0,
    230.299 / 255.0, 39.2759 / 255.0, 5.54531 / 255.0, 1.0,
    242.924 / 255.0, 94.3563 / 255.0, 22.4186 / 255.0, 1.0,
    242.435 / 255.0, 156.752 / 255.0, 58.9794 / 255.0, 1.0,
    135.291 / 255.0, 152.793 / 255.0, 182.473 / 255.0, 1.0,
];

#[rustfmt::skip]
pub static COLOR_SCHEME_POOLSIDE: [f32; 24] = [
    76.0 / 255.0, 156.0 / 255.0, 228.0 / 255.0, 1.0,
    140.0 / 255.0, 204.0 / 255.0, 244.0 / 255.0, 1.0,
    108.0 / 255.0, 180.0 / 255.0, 236.0 / 255.0, 1.0,
    188.0 / 255.0, 228.0 / 255.0, 244.0 / 255.0, 1.0,
    124.0 / 255.0, 220.0 / 255.0, 236.0 / 255.0, 1.0,
    156.0 / 255.0, 208.0 / 255.0, 236.0 / 255.0, 1.0,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_presets_retain_their_units_and_default_to_100_percent() {
        let legacy = serde::de::value::MapDeserializer::<_, serde::de::value::Error>::new(
            [
                ("lineLength", 450.0),
                ("lineWidth", 9.0),
                ("viewScale", 1.6),
            ]
            .into_iter(),
        );
        let settings = Settings::deserialize(legacy).unwrap();
        assert_eq!(settings.line_length, 450.0);
        assert_eq!(settings.line_width, 9.0);
        assert_eq!(settings.grid_spacing, 15);
        assert_eq!(settings.overall_scale(), 1.0);
        assert!((settings.line_length_pixels() - 293.87756).abs() < 0.0001);
        assert!((settings.line_width_pixels() - 5.877551).abs() < 0.0001);
    }

    #[test]
    fn overall_size_scales_dimensions_without_changing_preset_values() {
        let settings = Settings {
            overall_scale: 2.0,
            ..Settings::default()
        };
        let original = Settings::default();
        assert_eq!(settings.line_length, original.line_length);
        assert_eq!(settings.line_width, original.line_width);
        assert_eq!(
            settings.line_length_pixels(),
            2.0 * original.line_length_pixels()
        );
        assert_eq!(
            settings.line_width_pixels(),
            2.0 * original.line_width_pixels()
        );
    }
}
