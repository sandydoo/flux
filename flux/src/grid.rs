#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScalingRatio {
    x: f32,
    y: f32,
}

impl ScalingRatio {
    pub fn new(columns: u32, rows: u32) -> Self {
        let x = (columns as f32 / 171.0).max(1.0);
        let y = (rows as f32 / 171.0).max(1.0);
        Self { x, y }
    }

    pub fn x(&self) -> f32 {
        self.x
    }

    pub fn y(&self) -> f32 {
        self.y
    }

    pub fn rounded_x(&self) -> u32 {
        self.x.round() as u32
    }

    pub fn rounded_y(&self) -> u32 {
        self.y.round() as u32
    }
}

pub struct Grid {
    pub width: u32,
    pub height: u32,
    pub aspect_ratio: f32,
    pub columns: u32,
    pub rows: u32,
    pub line_count: u32,
    pub scaling_ratio: ScalingRatio,
    pub basepoints: Vec<f32>,
}

impl Grid {
    pub fn new(uwidth: u32, uheight: u32, grid_spacing: u32) -> Self {
        let height = uheight as f32;
        let width = uwidth as f32;
        let aspect_ratio = width / height;
        let grid_spacing = grid_spacing as f32;

        // The grid is a centred lattice of odd size (a line sits exactly at the
        // centre). `half_columns` is the number of cells from centre to edge on
        // each axis; the counts depend only on their own axis, and change in
        // steps of ±1 cell per edge as the window or spacing changes.
        let half_columns = (width / (2.0 * grid_spacing)).round().max(1.0) as u32;
        let half_rows = (height / (2.0 * grid_spacing)).round().max(1.0) as u32;
        let columns = 2 * half_columns + 1;
        let rows = 2 * half_rows + 1;
        let line_count = rows * columns;
        let scaling_ratio = ScalingRatio::new(columns, rows);

        // Cell spacing in the normalized [0,1] grid space. Crucially this is
        // continuous in the window size (`grid_spacing / window`), not the
        // reciprocal of the (quantized) cell count. A line at centre-offset `d`
        // then sits at a fixed on-screen distance from the centre regardless of
        // window size, so the grid holds its position as the window resizes
        // instead of stretching to fill a fixed fraction of it and snapping back
        // each time the count changes. Because the view is zoomed in, the count
        // changes land in the off-screen margin: cells appear/disappear at the
        // edges while everything visible stays put. `render::lines` resamples
        // line state by centre-offset to preserve each line's identity, and
        // refreshes the (window-dependent) basepoints on every resize.
        let grid_spacing_x = grid_spacing / width;
        let grid_spacing_y = grid_spacing / height;

        let mut basepoints = Vec::with_capacity(2 * line_count as usize);

        for v in 0..rows {
            for u in 0..columns {
                basepoints.push(0.5 + (u as f32 - half_columns as f32) * grid_spacing_x);
                basepoints.push(0.5 + (v as f32 - half_rows as f32) * grid_spacing_y);
            }
        }

        Self {
            width: uwidth,
            height: uheight,
            aspect_ratio,
            columns,
            rows,
            scaling_ratio,
            line_count,
            basepoints,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn clamp_logical_size(width: u32, height: u32) -> (u32, u32) {
        let width = width as f32;
        let height = height as f32;

        // TODO: Should we also clamp the upper bound?
        let minimum_dimension = 800.0;
        let scale = f32::max(minimum_dimension / width, minimum_dimension / height).max(1.0);
        (
            (width * scale).floor() as u32,
            (height * scale).floor() as u32,
        )
    }

    #[derive(Copy, Clone, PartialEq, Debug)]
    struct LogicalSize {
        pub width: u32,
        pub height: u32,
    }

    impl LogicalSize {
        pub fn new(width: u32, height: u32) -> Self {
            Self { width, height }
        }
    }

    fn create_test_grid(logical_size: LogicalSize, grid_spacing: u32) -> (u32, u32) {
        let Grid { columns, rows, .. } =
            Grid::new(logical_size.width, logical_size.height, grid_spacing);
        (columns, rows)
    }

    #[test]
    fn is_sane_grid_for_iphone_xr() {
        let logical_size = LogicalSize::new(414, 896);
        assert_eq!(create_test_grid(logical_size, 15), (29, 61));
        assert_eq!(
            clamp_logical_size(logical_size.width, logical_size.height),
            (800, 1731)
        );
    }

    #[test]
    fn is_sane_grid_for_iphone_12_pro() {
        let logical_size = LogicalSize::new(390, 844);
        assert_eq!(create_test_grid(logical_size, 15), (27, 57));
        assert_eq!(
            clamp_logical_size(logical_size.width, logical_size.height),
            (800, 1731)
        );
    }

    #[test]
    fn is_sane_grid_for_macbook_pro_13_with_1280_800_scaling() {
        let logical_size = LogicalSize::new(1280, 800);
        assert_eq!(create_test_grid(logical_size, 15), (87, 55));
        assert_eq!(
            clamp_logical_size(logical_size.width, logical_size.height),
            (1280, 800)
        );
    }

    #[test]
    fn is_sane_grid_for_macbook_pro_15_with_1440_900_scaling() {
        let logical_size = LogicalSize::new(1440, 900);
        assert_eq!(create_test_grid(logical_size, 15), (97, 61));
        assert_eq!(
            clamp_logical_size(logical_size.width, logical_size.height),
            (1440, 900)
        );
    }

    #[test]
    fn is_sane_grid_for_ultrawide_4k() {
        let logical_size = LogicalSize::new(3840, 1600);
        assert_eq!(create_test_grid(logical_size, 15), (257, 107));
        assert_eq!(
            clamp_logical_size(logical_size.width, logical_size.height),
            (3840, 1600)
        );
    }

    #[test]
    fn is_sane_grid_for_triple_2560_1440() {
        let logical_size = LogicalSize::new(2560 * 3, 1440);
        assert_eq!(create_test_grid(logical_size, 15), (513, 97));
        assert_eq!(
            clamp_logical_size(logical_size.width, logical_size.height),
            (logical_size.width, logical_size.height)
        );
    }
}
