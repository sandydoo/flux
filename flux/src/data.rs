use std::f32::consts::PI;

// Common geometries

pub static PLANE_INDICES: [u16; 6] = [0, 1, 2, 2, 3, 0];
pub static PLANE_VERTICES: [f32; 12] = [
    1.0, -1.0, 0.0, //
    1.0, 1.0, 0.0, //
    -1.0, 1.0, 0.0, //
    -1.0, -1.0, 0.0, //
];

pub static LINE_VERTICES: [f32; 12] = [
    0.0, -0.5, //
    1.0, -0.5, //
    1.0, 0.5, //
    0.0, -0.5, //
    1.0, 0.5, //
    0.0, 0.5,
];

// Points

// World space coordinates: zero-centered, width x height
pub fn new_points(width: u32, height: u32, grid_spacing: u32) -> Vec<f32> {
    let half_width = (width as f32) / 2.0;
    let half_height = (height as f32) / 2.0;

    let rows = height / grid_spacing;
    let cols = width / grid_spacing;
    let mut data = Vec::with_capacity((rows * cols * 2) as usize);

    for v in 0..rows {
        for u in 0..cols {
            let x: f32 = (u * grid_spacing) as f32;
            let y: f32 = (v * grid_spacing) as f32;

            data.push(x - half_width);
            data.push(y - half_height);
        }
    }

    data
}

// World space coordinates: zero-centered, width x height
pub fn new_line_state(width: u32, height: u32, grid_spacing: u32) -> Vec<f32> {
    let rows = height / grid_spacing;
    let cols = width / grid_spacing;
    let mut data = Vec::with_capacity((rows * cols * 10) as usize);

    for _ in 0..rows {
        for _ in 0..cols {
            // endpoint
            data.push(0.0001);
            data.push(0.0001);

            // velocity
            data.push(0.0);
            data.push(0.0);

            // color
            data.push(0.0);
            data.push(0.0);
            data.push(0.0);
            data.push(0.0); // not currently used

            // width
            data.push(0.0);

            // opacity
            data.push(0.0);
        }
    }

    data
}

pub fn new_semicircle(resolution: u32) -> Vec<f32> {
    let mut segments = Vec::with_capacity((resolution * 2 + 1) as usize);

    segments.push(0.0);
    segments.push(0.0);

    for section in 0..=resolution {
        let angle = PI * (section as f32) / (resolution as f32);
        segments.push(angle.cos());
        segments.push(angle.sin());
    }

    segments
}