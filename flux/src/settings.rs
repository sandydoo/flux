use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub viscosity: f32,
    pub velocity_dissipation: f32,
    pub fluid_width: u32,
    pub fluid_height: u32,
    pub diffusion_iterations: u32,
    pub pressure_iterations: u32,

    pub color_scheme: ColorScheme,

    pub line_length: f32,
    pub line_width: f32,
    pub line_begin_offset: f32,
    pub adjust_advection: f32,

    pub noise_channel_1: Noise,
    pub noise_channel_2: Noise,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ColorScheme {
    Plasma,
    Poolside,
    Pollen,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Noise {
    pub scale: f32,
    pub multiplier: f32,
    pub offset_1: f32,
    pub offset_2: f32,
    pub offset_increment: f32,
    pub blend_duration: f32,
}

pub fn color_wheel_from_scheme(color_scheme: &ColorScheme) -> [f32; 18] {
    match color_scheme {
        ColorScheme::Plasma => COLOR_SCHEME_PLASMA,
        ColorScheme::Poolside => COLOR_SCHEME_POOLSIDE,
        ColorScheme::Pollen => COLOR_SCHEME_POLLEN,
    }
}

#[rustfmt::skip]
pub static COLOR_SCHEME_PLASMA: [f32; 18] = [
    60.219  / 255.0, 37.2487 / 255.0, 66.4301 / 255.0,
    170.962 / 255.0, 54.4873 / 255.0, 50.9661 / 255.0,
    230.299 / 255.0, 39.2759 / 255.0, 5.54531 / 255.0,
    242.924 / 255.0, 94.3563 / 255.0, 22.4186 / 255.0,
    242.435 / 255.0, 156.752 / 255.0, 58.9794 / 255.0,
    135.291 / 255.0, 152.793 / 255.0, 182.473 / 255.0,
];
#[rustfmt::skip]
pub static COLOR_SCHEME_POOLSIDE: [f32; 18] = [
    76.0  / 255.0, 158.0 / 255.0 , 226.0 / 255.0,
    108.0 / 255.0, 180.0 / 255.0 , 233.0 / 255.0,
    139.0 / 255.0, 201.0 / 255.0 , 240.0 / 255.0,
    188.0 / 255.0, 226.0 / 255.0 , 247.0 / 255.0,
    76.0  / 255.0, 158.0 / 255.0 , 226.0 / 255.0,
    108.0 / 255.0, 180.0 / 255.0 , 233.0 / 255.0,

];
#[rustfmt::skip]
pub static COLOR_SCHEME_POLLEN: [f32; 18] = [
    0.98431373, 0.71764706, 0.19215686,
    0.98431373, 0.71764706, 0.19215686,
    0.98431373, 0.71764706, 0.19215686,
    0.98431373, 0.71764706, 0.19215686,
    0.98431373, 0.71764706, 0.19215686,
    0.98431373, 0.71764706, 0.19215686,
];
