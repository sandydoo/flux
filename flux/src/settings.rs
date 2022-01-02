use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub viscosity: f32,
    pub velocity_dissipation: f32,
    pub adjust_advection: f32,
    pub fluid_width: u32,
    pub fluid_height: u32,
    pub diffusion_iterations: u32,
    pub pressure_iterations: u32,

    pub line_length: f32,
    pub line_width: f32,
    pub line_begin_offset: f32,

    pub noise_channel_1: Noise,
    pub noise_channel_2: Noise,
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
