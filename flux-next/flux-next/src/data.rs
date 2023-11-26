// Common geometries

#[rustfmt::skip]
pub static PLANE_VERTICES: [f32; 12] = [
     1.0, -1.0,
     1.0,  1.0,
    -1.0,  1.0,
    -1.0,  1.0,
    -1.0, -1.0,
    1.0, -1.0,
];

#[rustfmt::skip]
pub static LINE_VERTICES: [f32; 12] = [
    -0.5, 0.0,
    -0.5, 1.0,
     0.5, 1.0,
    -0.5, 0.0,
     0.5, 1.0,
     0.5, 0.0,
];
