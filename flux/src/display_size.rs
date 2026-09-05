/// Logical scene dimensions and the independently limited backing allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplaySize {
    pub logical_width: u32,
    pub logical_height: u32,
    pub physical_width: u32,
    pub physical_height: u32,
}

impl DisplaySize {
    pub fn from_logical(width: u32, height: u32, scale: f64, limit: u32) -> Option<Self> {
        if width == 0 || height == 0 || limit == 0 {
            return None;
        }
        let scale = valid_scale(scale).min(f64::from(limit) / f64::from(width.max(height)));
        Some(Self {
            logical_width: width,
            logical_height: height,
            physical_width: (f64::from(width) * scale)
                .round()
                .clamp(1.0, f64::from(limit)) as u32,
            physical_height: (f64::from(height) * scale)
                .round()
                .clamp(1.0, f64::from(limit)) as u32,
        })
    }

    pub fn from_physical(width: u32, height: u32, scale: f64, limit: u32) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        let scale = valid_scale(scale);
        // Preserve the actual window aspect ratio in the backing allocation;
        // rounding logical dimensions must not change the physical size.
        let backing = Self::from_logical(width, height, 1.0, limit)?;
        Some(Self {
            logical_width: (f64::from(width) / scale).round().max(1.0) as u32,
            logical_height: (f64::from(height) / scale).round().max(1.0) as u32,
            ..backing
        })
    }
}

fn valid_scale(scale: f64) -> f64 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backing_scale_only_changes_sharpness() {
        let low = DisplaySize::from_logical(1200, 800, 1.0, 8192).unwrap();
        let high = DisplaySize::from_logical(1200, 800, 2.0, 8192).unwrap();
        assert_eq!(
            (low.logical_width, low.logical_height),
            (high.logical_width, high.logical_height)
        );
        assert_eq!((high.physical_width, high.physical_height), (2400, 1600));
        assert_eq!(
            DisplaySize::from_physical(2400, 1600, 2.0, 8192),
            Some(high)
        );
    }

    #[test]
    fn limits_backing_dimensions_without_changing_scene_or_aspect() {
        let size = DisplaySize::from_logical(6000, 3000, 3.0, 8192).unwrap();
        assert_eq!((size.logical_width, size.logical_height), (6000, 3000));
        assert_eq!((size.physical_width, size.physical_height), (8192, 4096));
        let native = DisplaySize::from_physical(18000, 9000, 3.0, 8192).unwrap();
        assert_eq!(native, size);
        // Once allocation is limited, further DPR changes require no resize.
        assert_eq!(DisplaySize::from_logical(6000, 3000, 4.0, 8192), Some(size));
    }

    #[test]
    fn native_scale_change_updates_logical_dimensions_at_same_physical_size() {
        let before = DisplaySize::from_physical(2400, 1600, 1.0, 8192).unwrap();
        let after = DisplaySize::from_physical(2400, 1600, 2.0, 8192).unwrap();
        assert_ne!(before, after);
        assert_eq!((after.logical_width, after.logical_height), (1200, 800));
        assert_eq!(
            (before.physical_width, before.physical_height),
            (after.physical_width, after.physical_height)
        );
    }

    #[test]
    fn zero_dimensions_suspend_and_tiny_nonzero_dimensions_stay_valid() {
        assert_eq!(DisplaySize::from_logical(0, 800, 2.0, 8192), None);
        assert_eq!(DisplaySize::from_physical(1200, 0, 2.0, 8192), None);
        assert_eq!(
            DisplaySize::from_physical(1, 1, 4.0, 8192)
                .unwrap()
                .logical_width,
            1
        );
        assert_eq!(
            DisplaySize::from_logical(1, 1, 0.1, 8192)
                .unwrap()
                .physical_height,
            1
        );
    }

    #[test]
    fn fractional_scale_and_invalid_scale_are_handled() {
        assert_eq!(
            DisplaySize::from_logical(801, 601, 1.25, 8192)
                .unwrap()
                .physical_width,
            1001
        );
        for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                DisplaySize::from_logical(800, 600, scale, 8192),
                DisplaySize::from_logical(800, 600, 1.0, 8192)
            );
        }
    }
}
