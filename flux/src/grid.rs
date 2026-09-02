/// How much larger than their base size the fluid and noise textures are,
/// per axis.
///
/// The ratio follows the shape of the grid, so the textures have the aspect
/// ratio of the window. Every simulation pass works in texel space, and a
/// texel then covers a square patch of the screen: vortices stay round on an
/// ultrawide display instead of being stretched across it.
///
/// The shorter side of the grid maps onto the base size. Once that side has
/// more cells than `REFERENCE_CELLS`, the textures grow with the grid so that
/// large displays keep about the same fluid detail per line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScalingRatio {
    x: f32,
    y: f32,
}

impl ScalingRatio {
    /// The grid side (in cells) that maps onto one base texture size before
    /// the textures start to grow with the grid.
    const REFERENCE_CELLS: f32 = 171.0;

    /// Texture sides are rounded to this. It is the compute workgroup size.
    const TEXTURE_ALIGNMENT: f32 = 16.0;

    pub fn new(columns: u32, rows: u32) -> Self {
        let shorter_side = columns.min(rows) as f32;
        let reference = shorter_side.clamp(1.0, Self::REFERENCE_CELLS);
        Self {
            x: columns as f32 / reference,
            y: rows as f32 / reference,
        }
    }

    pub fn x(&self) -> f32 {
        self.x
    }

    pub fn y(&self) -> f32 {
        self.y
    }

    /// The size of a texture with the base size `base` on its shorter side.
    pub fn texture_size(&self, base: u32) -> wgpu::Extent3d {
        let align = |side: f32| {
            let steps = (side / Self::TEXTURE_ALIGNMENT).round().max(1.0);
            (steps * Self::TEXTURE_ALIGNMENT) as u32
        };
        wgpu::Extent3d {
            width: align(base as f32 * self.x),
            height: align(base as f32 * self.y),
            depth_or_array_layers: 1,
        }
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

    fn fluid_size(width: u32, height: u32, grid_spacing: u32) -> (u32, u32) {
        let grid = Grid::new(width, height, grid_spacing);
        let size = grid.scaling_ratio.texture_size(128);
        (size.width, size.height)
    }

    #[test]
    fn fluid_keeps_the_aspect_ratio_of_the_window() {
        // 3:2, 16:9, 21:9, and portrait.
        assert_eq!(fluid_size(1200, 800, 15), (192, 128));
        assert_eq!(fluid_size(2560, 1440, 15), (224, 128));
        assert_eq!(fluid_size(3840, 1600, 15), (304, 128));
        assert_eq!(fluid_size(414, 896, 15), (128, 272));
    }

    #[test]
    fn fluid_grows_with_large_grids() {
        // The shorter side has more than REFERENCE_CELLS cells.
        assert_eq!(fluid_size(5120, 2880, 15), (256, 144));
        assert_eq!(fluid_size(2560 * 3, 1440, 15), (672, 128));
        assert_eq!(fluid_size(1200, 800, 5), (192, 128));
        assert_eq!(fluid_size(3840, 2160, 5), (576, 320));
    }

    #[test]
    fn fluid_has_a_minimum_size() {
        assert_eq!(fluid_size(16, 16, 15), (128, 128));
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
