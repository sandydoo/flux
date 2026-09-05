/// Logical pixels in one world-space reference height.
pub const REFERENCE_HEIGHT: f32 = 800.0;

/// Centered logical world-domain size, independent of simulation quality.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScalingRatio {
    x: f32,
    y: f32,
}

/// Caps aspect-driven texture growth to sixteen base areas and device limits.
#[derive(Clone, Copy, Debug)]
pub struct TextureBudget {
    max_texels: u64,
    max_dimension: u32,
}

impl TextureBudget {
    const MAX_BASE_AREA_MULTIPLIER: u64 = 16;

    pub fn for_base(base: u32, max_dimension: u32) -> Self {
        assert!(max_dimension >= 16, "device must support a 16×16 workgroup");
        let base_area = u64::from(base).saturating_mul(u64::from(base));
        Self {
            // Preserve the existing minimum 16x16 texture for a zero base.
            max_texels: base_area
                .saturating_mul(Self::MAX_BASE_AREA_MULTIPLIER)
                .max(16 * 16),
            max_dimension,
        }
    }
}

impl ScalingRatio {
    /// Texture sides are rounded to this. It is the compute workgroup size.
    const TEXTURE_ALIGNMENT: u32 = 16;

    /// The ratio for a surface of the given logical size.
    pub fn new(width: u32, height: u32) -> Self {
        let reference = REFERENCE_HEIGHT;
        Self {
            x: width as f32 / reference,
            y: height as f32 / reference,
        }
    }

    pub fn x(&self) -> f32 {
        self.x
    }

    pub fn y(&self) -> f32 {
        self.y
    }

    /// Coordinates used by the original tuned square simulation field.
    /// A 1280×800 reference frame spans one unit along each axis. Keep this
    /// calibration separate from the logical world used for centered resizing.
    pub fn simulation_domain(self) -> [f32; 2] {
        [self.x / 1.6, self.y]
    }

    /// Change the visible world extent without changing texture quality.
    pub fn with_scale(self, scale: f32) -> Self {
        Self {
            x: self.x / scale,
            y: self.y / scale,
        }
    }

    /// A reference 16:10 surface gets a coarse base×base field regardless of
    /// pixel dimensions. Wider or taller spans extend only that axis. The
    /// solver uses world cell metrics; the texture need not have square cells.
    pub fn texture_size(&self, base: u32, budget: TextureBudget) -> wgpu::Extent3d {
        // Work in whole workgroups, using f64 before conversion so extreme
        // requested sizes cannot overflow or saturate an intermediate u32.
        let alignment = Self::TEXTURE_ALIGNMENT;
        let groups = |ratio: f32| {
            (f64::from(base) * f64::from(ratio) / f64::from(alignment))
                .round()
                .max(1.0)
        };
        let aspect =
            f64::from(self.x.max(f32::MIN_POSITIVE)) / f64::from(self.y.max(f32::MIN_POSITIVE));
        let desired_width = groups((aspect / 1.6).max(1.0) as f32);
        let desired_height = groups((1.6 / aspect).max(1.0) as f32);
        let max_side = budget.max_dimension / alignment;
        let max_area = budget.max_texels / u64::from(alignment).pow(2);
        let scale = (f64::from(max_side) / desired_width.max(desired_height))
            .min((max_area as f64 / (desired_width * desired_height)).sqrt())
            .min(1.0);
        let mut width = ((desired_width * scale).round() as u32).clamp(1, max_side);
        let mut height = ((desired_height * scale).round() as u32).clamp(1, max_side);

        // Rounding and the one-workgroup minimum can exceed the area cap.
        // Choose the axis reduction that better preserves the desired shape.
        if u64::from(width) * u64::from(height) > max_area {
            let narrower = (max_area / u64::from(height)).max(1) as u32;
            let shorter = (max_area / u64::from(width)).max(1) as u32;
            let target_aspect = desired_width / desired_height;
            let width_error = ((f64::from(narrower) / f64::from(height)) / target_aspect)
                .ln()
                .abs();
            let height_error = ((f64::from(width) / f64::from(shorter)) / target_aspect)
                .ln()
                .abs();
            if u64::from(narrower) * u64::from(height) <= max_area && width_error <= height_error {
                width = narrower;
            } else {
                height = shorter;
                width = u64::from(width).min(max_area / u64::from(height)) as u32;
            }
        }

        wgpu::Extent3d {
            width: width * alignment,
            height: height * alignment,
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
    // Two 48-byte line-state buffers plus two 8-byte basepoint buffers stay
    // below 28 MiB. Also bounds the CPU allocation and compute dispatch size.
    pub const MAX_LINE_COUNT: u32 = 262_144;

    pub fn new(uwidth: u32, uheight: u32, grid_spacing: u32) -> Self {
        Self::with_scale(uwidth, uheight, grid_spacing, 1.0)
    }

    pub fn with_scale(uwidth: u32, uheight: u32, grid_spacing: u32, overall_scale: f32) -> Self {
        let uwidth = uwidth.max(1);
        let uheight = uheight.max(1);
        let height = uheight as f32;
        let width = uwidth as f32;
        let aspect_ratio = width / height;
        let overall_scale = if overall_scale.is_finite() && overall_scale > 0.0 {
            overall_scale.clamp(0.1, 10.0)
        } else {
            1.0
        };
        let mut grid_spacing = f64::from(grid_spacing.max(1)) * f64::from(overall_scale);

        // The grid is a centred lattice of odd size (a line sits exactly at the
        // centre). `half_columns` is the number of cells from centre to edge on
        // each axis; the counts depend only on their own axis, and change in
        // steps of ±1 cell per edge as the window or spacing changes.
        let half_cells =
            |side: u32, spacing: f64| (f64::from(side) / (2.0 * spacing)).round().max(1.0);
        // Increase both axes' spacing together on extremely dense or large
        // surfaces. Compute counts in f64 before conversion to avoid overflow.
        loop {
            let count = (2.0 * half_cells(uwidth, grid_spacing) + 1.0)
                * (2.0 * half_cells(uheight, grid_spacing) + 1.0);
            if count <= f64::from(Self::MAX_LINE_COUNT) {
                break;
            }
            grid_spacing *= (count / f64::from(Self::MAX_LINE_COUNT)).sqrt().max(1.01);
        }
        let half_columns = half_cells(uwidth, grid_spacing) as u32;
        let half_rows = half_cells(uheight, grid_spacing) as u32;
        let columns = 2 * half_columns + 1;
        let rows = 2 * half_rows + 1;
        let line_count = rows * columns;
        let scaling_ratio = ScalingRatio::new(uwidth, uheight).with_scale(overall_scale);

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
        let grid_spacing_x = grid_spacing as f32 / width;
        let grid_spacing_y = grid_spacing as f32 / height;

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

    fn center_offset(grid: &Grid, x: u32, y: u32) -> (f32, f32) {
        let index =
            (((grid.rows - 1) / 2 + y) * grid.columns + (grid.columns - 1) / 2 + x) as usize * 2;
        (
            (grid.basepoints[index] - 0.5) * grid.width as f32,
            (grid.basepoints[index + 1] - 0.5) * grid.height as f32,
        )
    }

    #[test]
    fn logical_spacing_is_fixed_across_sizes_and_overall_scales() {
        for (width, height) in [
            (390, 844),
            (1280, 800),
            (2560, 800),
            (800, 2560),
            (3840, 2160),
        ] {
            for scale in [0.5, 1.0, 1.5, 2.0] {
                let grid = Grid::with_scale(width, height, 15, scale);
                let (x, y) = center_offset(&grid, 1, 1);
                assert!((x - 15.0 * scale).abs() < 0.001);
                assert!((y - 15.0 * scale).abs() < 0.001);
                assert_eq!(center_offset(&grid, 0, 0), (0.0, 0.0));
                assert_eq!(
                    grid.scaling_ratio,
                    ScalingRatio::new(width, height).with_scale(scale)
                );
            }
        }
    }

    #[test]
    fn small_spacing_changes_move_targets_without_changing_line_identity() {
        let before = Grid::with_scale(1280, 800, 15, 1.0);
        let after = Grid::with_scale(1280, 800, 15, 1.001);
        assert_eq!((before.columns, before.rows), (after.columns, after.rows));
        assert_eq!(center_offset(&before, 0, 0), center_offset(&after, 0, 0));
        let (before_x, before_y) = center_offset(&before, 1, 1);
        let (after_x, after_y) = center_offset(&after, 1, 1);
        assert!((after_x / before_x - 1.001).abs() < 0.00001);
        assert!((after_y / before_y - 1.001).abs() < 0.00001);
    }

    #[test]
    fn degenerate_and_extreme_layouts_are_finite_and_bounded() {
        for (width, height, spacing, scale) in [
            (0, 0, 0, 0.0),
            (0, 800, 0, f32::NAN),
            (1280, 0, 15, f32::INFINITY),
            (u32::MAX, u32::MAX, 1, 0.1),
            (u32::MAX, 1, 1, 1.0),
            (1, u32::MAX, 1, 1.0),
        ] {
            let grid = Grid::with_scale(width, height, spacing, scale);
            assert!(grid.aspect_ratio.is_finite() && grid.aspect_ratio > 0.0);
            assert!(grid.line_count <= Grid::MAX_LINE_COUNT);
            assert_eq!(grid.basepoints.len(), 2 * grid.line_count as usize);
            assert!(grid.basepoints.iter().all(|value| value.is_finite()));
            assert_eq!(grid.columns % 2, 1);
            assert_eq!(grid.rows % 2, 1);
        }
    }

    fn grid_counts(width: u32, height: u32, spacing: u32) -> (u32, u32) {
        let Grid { columns, rows, .. } = Grid::new(width, height, spacing);
        (columns, rows)
    }

    #[test]
    fn is_sane_grid_for_iphone_xr() {
        assert_eq!(grid_counts(414, 896, 15), (29, 61));
    }

    #[test]
    fn is_sane_grid_for_iphone_12_pro() {
        assert_eq!(grid_counts(390, 844, 15), (27, 57));
    }

    #[test]
    fn is_sane_grid_for_macbook_pro_13_with_1280_800_scaling() {
        assert_eq!(grid_counts(1280, 800, 15), (87, 55));
    }

    #[test]
    fn is_sane_grid_for_macbook_pro_15_with_1440_900_scaling() {
        assert_eq!(grid_counts(1440, 900, 15), (97, 61));
    }

    #[test]
    fn is_sane_grid_for_ultrawide_4k() {
        assert_eq!(grid_counts(3840, 1600, 15), (257, 107));
    }

    fn fluid_size(width: u32, height: u32, grid_spacing: u32) -> (u32, u32) {
        let grid = Grid::new(width, height, grid_spacing);
        let size = grid
            .scaling_ratio
            .texture_size(128, TextureBudget::for_base(128, u32::MAX));
        (size.width, size.height)
    }

    #[test]
    fn scaling_ratio_counts_reference_displays() {
        assert_eq!(
            ScalingRatio::new(1280, 800),
            ScalingRatio { x: 1.6, y: 1.0 }
        );
        assert_eq!(
            ScalingRatio::new(2560, 800),
            ScalingRatio { x: 3.2, y: 1.0 }
        );
        assert_eq!(
            ScalingRatio::new(1280, 1600),
            ScalingRatio { x: 1.6, y: 2.0 }
        );
        assert_eq!(ScalingRatio::new(640, 400), ScalingRatio { x: 0.8, y: 0.5 });
    }

    #[test]
    fn fluid_has_the_base_size_on_the_reference_display() {
        assert_eq!(fluid_size(1280, 800, 15), (128, 128));
    }

    #[test]
    fn fluid_extends_with_the_surface() {
        // Spans extend the relevant axis; a half-size window stays coarse.
        assert_eq!(fluid_size(2560, 800, 15), (256, 128));
        assert_eq!(fluid_size(1280, 1600, 15), (128, 256));
        assert_eq!(fluid_size(640, 400, 15), (128, 128));
    }

    #[test]
    fn fluid_quality_is_independent_of_absolute_dimensions() {
        assert_eq!(fluid_size(1280, 800, 15), fluid_size(3840, 2400, 15));
        assert_eq!(fluid_size(640, 400, 15), fluid_size(2560, 1600, 15));
        let ratio = ScalingRatio::new(2560, 800);
        let budget = TextureBudget::for_base(128, 8192);
        assert_eq!(
            ratio.texture_size(128, budget),
            ratio.with_scale(2.0).texture_size(128, budget)
        );
    }

    #[test]
    fn fluid_does_not_depend_on_the_grid_spacing() {
        assert_eq!(fluid_size(1280, 800, 5), fluid_size(1280, 800, 15));
        assert_eq!(fluid_size(3840, 2160, 5), fluid_size(3840, 2160, 30));
    }

    #[test]
    fn fluid_extends_relative_to_reference_aspect() {
        // 3:2, 16:9, 21:9, and portrait.
        assert_eq!(fluid_size(1200, 800, 15), (128, 144));
        assert_eq!(fluid_size(2560, 1440, 15), (144, 128));
        assert_eq!(fluid_size(3840, 1600, 15), (192, 128));
        assert_eq!(fluid_size(414, 896, 15), (128, 448));
    }

    #[test]
    fn tiny_square_surface_keeps_aspect_based_fluid_quality() {
        assert_eq!(fluid_size(16, 16, 15), (128, 208));
    }

    #[test]
    fn fluid_is_bounded_on_dense_and_spanned_surfaces() {
        let grid = Grid::new(3840 * 3, 2160, 5);
        let size = fluid_size(3840 * 3, 2160, 5);
        assert!(u64::from(size.0) * u64::from(size.1) <= 128 * 128 * 16);

        let target_aspect = grid.width as f32 / grid.height as f32 / 1.6;
        let actual_aspect = size.0 as f32 / size.1 as f32;
        assert!((actual_aspect / target_aspect - 1.0).abs() < 0.05);
    }

    #[test]
    fn texture_size_respects_the_device_limit() {
        let ratio = ScalingRatio::new(10_001, 101);
        let size = ratio.texture_size(128, TextureBudget::for_base(128, 256));
        assert!(size.width <= 256);
        assert!(size.height <= 256);
    }

    #[test]
    fn texture_budget_handles_extreme_and_degenerate_requests() {
        for base in [0, 1, 128, u32::MAX] {
            for limit in [16, 255, 8192, u32::MAX] {
                let budget = TextureBudget::for_base(base, limit);
                for (width, height) in [(0, 0), (0, u32::MAX), (u32::MAX, 1), (u32::MAX, u32::MAX)]
                {
                    let size = ScalingRatio::new(width, height).texture_size(base, budget);
                    assert!(size.width >= 16 && size.height >= 16);
                    assert_eq!(size.width % 16, 0);
                    assert_eq!(size.height % 16, 0);
                    assert!(size.width <= limit && size.height <= limit);
                    assert!(u64::from(size.width) * u64::from(size.height) <= budget.max_texels);
                }
            }
        }
    }

    #[test]
    fn is_sane_grid_for_triple_2560_1440() {
        assert_eq!(grid_counts(2560 * 3, 1440, 15), (513, 97));
    }
}
