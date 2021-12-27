use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
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
}

