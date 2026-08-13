// The earth model, stated once for the whole program: a sphere of the IUGG mean radius, with
// distances as great-circle arcs on it. Nothing here is ellipsoidal, so a result may differ from a
// WGS 84 geodesic by a few tenths of a percent.
pub const EARTH_RADIUS_KM: f64 = 6371.0088;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

/// Longitude reduced to [-180, 180). Latitude has no counterpart here on purpose: a coordinate past
/// a pole is an error to reject, not a value to fold, because folding it silently moves a point to
/// the far side of the globe.
#[must_use]
pub fn wrap_lon(lon: f64) -> f64 {
    (lon + 180.0).rem_euclid(360.0) - 180.0
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this
// narrow exemption; docs/ai/code.md allows both in tests.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn the_seam_folds_west() {
        // A half-open range has to choose an end, and -180 is the one that exists. All three of
        // these name the same meridian.
        for lon in [180.0, -180.0, 540.0, -540.0] {
            assert!((wrap_lon(lon) - -180.0).abs() < 1e-12, "wrap_lon({lon})");
        }
    }

    #[test]
    fn longitudes_already_in_range_are_untouched() {
        for lon in [-180.0, -179.5, -0.5, 0.0, 0.5, 179.5] {
            assert!((wrap_lon(lon) - lon).abs() < 1e-12, "wrap_lon({lon})");
        }
    }

    proptest! {
        #[test]
        fn wrapping_always_lands_in_the_half_open_range(lon in -1.0e6f64..1.0e6) {
            let wrapped = wrap_lon(lon);
            prop_assert!((-180.0..180.0).contains(&wrapped), "{} wrapped to {}", lon, wrapped);
        }

        #[test]
        fn wrapping_is_invariant_under_whole_turns(lon in -180.0f64..180.0, k in -3i32..=3) {
            let shifted = wrap_lon(lon + 360.0 * f64::from(k));
            prop_assert!((shifted - wrap_lon(lon)).abs() < 1e-9, "{} vs {}", shifted, wrap_lon(lon));
        }
    }
}
