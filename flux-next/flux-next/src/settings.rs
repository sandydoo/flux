use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub mode: Mode,
    pub seed: Option<String>,

    pub fluid_size: u32,
    pub fluid_frame_rate: f32,
    pub fluid_timestep: f32,
    pub viscosity: f32,
    pub velocity_dissipation: f32,
    pub pressure_mode: PressureMode,
    pub diffusion_iterations: u32,
    pub pressure_iterations: u32,

    pub color_mode: ColorMode,

    pub line_length: f32,
    pub line_width: f32,
    pub line_begin_offset: f32,
    pub line_variance: f32,
    pub grid_spacing: u32,
    pub view_scale: f32,

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
            pressure_mode: PressureMode::Retain,
            diffusion_iterations: 3,
            pressure_iterations: 19,
            color_mode: ColorMode::Preset(ColorPreset::Original),
            line_length: 550.0,
            line_width: 10.0,
            line_begin_offset: 0.4,
            line_variance: 0.45,
            grid_spacing: 15,
            view_scale: 1.6,
            noise_channels: vec![
                Noise {
                    scale: 2.5,
                    multiplier: 1.0,
                    offset_increment: 0.0015,
                },
                Noise {
                    scale: 15.0,
                    multiplier: 0.7,
                    offset_increment: 0.0015 * 6.0,
                },
                Noise {
                    scale: 30.0,
                    multiplier: 0.5,
                    offset_increment: 0.0015 * 12.0,
                },
            ],
        }
    }
}

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
pub enum Mode {
    #[default]
    Normal,
    DebugNoise,
    DebugFluid,
    DebugPressure,
    DebugDivergence,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum PressureMode {
    Retain,
    ClearWith(f32),
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

#[derive(Copy, Clone, Default, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum ColorPreset {
    #[default]
    Original,
    Plasma,
    Poolside,
    Freedom,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Noise {
    pub scale: f32,
    pub multiplier: f32,
    pub offset_increment: f32,
}

pub fn color_wheel_from_mode(color_mode: &ColorMode) -> [f32; 24] {
    match color_mode {
        ColorMode::Preset(color_preset) => match color_preset {
            ColorPreset::Plasma => COLOR_SCHEME_PLASMA,
            ColorPreset::Poolside => COLOR_SCHEME_POOLSIDE,
            ColorPreset::Freedom => COLOR_SCHEME_FREEDOM,
            _ => [0.0; 24],
        },
        _ => [0.0; 24],
    }
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
#[rustfmt::skip]
pub static COLOR_SCHEME_FREEDOM: [f32; 24] = [
    0.0 / 255.0,   87.0 / 255.0,  183.0 / 255.0, 1.0, // blue
    0.0 / 255.0,   87.0 / 255.0,  183.0 / 255.0, 1.0, // blue
    0.0 / 255.0,   87.0 / 255.0,  183.0 / 255.0, 1.0, // blue
    1.0,           215.0 / 255.0, 0.0,           1.0, // yellow
    1.0,           215.0 / 255.0, 0.0,           1.0, // yellow
    1.0,           215.0 / 255.0, 0.0,           1.0, // yellow
];
