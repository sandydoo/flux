use std::f32::consts::PI;

// Common geometries

pub static PLANE_INDICES: [u16; 6] = [0, 1, 2, 2, 3, 0];
pub static PLANE_VERTICES: [f32; 12] = [
    1.0, -1.0, 0.0, //
    1.0, 1.0, 0.0, //
    -1.0, 1.0, 0.0, //
    -1.0, -1.0, 0.0, //
];

pub static LINE_INDICES: [u16; 6] = [0, 1, 2, 2, 3, 0];
pub static LINE_VERTICES: [f32; 12] = [
    0.0, -0.5, //
    1.0, -0.5, //
    1.0, 0.5, //
    0.0, -0.5, //
    1.0, 0.5, //
    0.0, 0.5,
];

// Points

pub fn new_points(width: u32, height: u32, grid_spacing: u32) -> Vec<f32> {
    let rows = height / grid_spacing;
    let cols = width / grid_spacing;
    let mut data = Vec::with_capacity((rows * cols * 4) as usize);

    for v in 0..rows {
        for u in 0..cols {
            let x: f32 = (u * grid_spacing) as f32;
            let y: f32 = (v * grid_spacing) as f32;
            data.push(x);
            data.push(y);
            data.push(0.0);
            data.push(1.0);
        }
    }

    data
}

pub fn new_line_state(width: u32, height: u32, grid_spacing: u32) -> Vec<f32> {
    let rows = height / grid_spacing;
    let cols = width / grid_spacing;
    let mut data = Vec::with_capacity((rows * cols * 4) as usize);

    for v in 0..rows {
        for u in 0..cols {
            let x: f32 = (u * grid_spacing) as f32;
            let y: f32 = (v * grid_spacing) as f32;
            data.push(x);
            data.push(y);
            data.push(0.0);
            data.push(0.0);
        }
    }

    data
}

pub fn make_sine_vector_field(rows: i32, cols: i32) -> Vec<f32> {
    let mut data = Vec::with_capacity((rows * cols * 4) as usize);
    let step_x = 1.0 / (rows as f32);
    let step_y = 1.0 / (cols as f32);

    for v in 0..cols {
        for u in 0..rows {
            let x = step_x * (u as f32) * 2.0 * -1.0;
            let y = step_y * (v as f32) * 2.0 * -1.0;

            // Swirrlies
            data.push(0.3 * (2.0 * PI * y).sin());
            data.push(0.3 * (2.0 * PI * x).sin());
            data.push(0.0);
            data.push(1.0);
        }
    }

    data
}

pub fn make_checkerboard_field(rows: i32, cols: i32) -> Vec<f32> {
    let mut data = Vec::with_capacity((rows * cols * 4) as usize);
    let step_x = 1.0 / (rows as f32);
    let step_y = 1.0 / (cols as f32);

    for u in 0..rows {
        for v in 0..cols {
            let offset_y = if (u + v) % 2 == 0 { v + 1 } else { v };

            let x: f32 = step_x * (u as f32) * 2.0 - 1.0;
            let y: f32 = step_y * (offset_y as f32) * 2.0 - 1.0;

            data.push(x);
            data.push(y);
            data.push(0.0);
            data.push(1.0);
        }
    }

    data
}

pub fn new_circle(resolution: u32) -> Vec<f32> {
    let mut segments = Vec::with_capacity((resolution * 2 + 1) as usize);

    segments.push(0.0);
    segments.push(0.0);

    for section in 0..=resolution {
        let angle = 2.0 * PI * (section as f32) / (resolution as f32);
        segments.push(1.0 * angle.cos());
        segments.push(1.0 * angle.sin());
    }

    segments
}
