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

/// The central angle between two points, by the haversine formula (Sinnott, "Virtues of the
/// Haversine", *Sky & Telescope* 68(2), 1984).
///
/// Haversine loses relative precision as the pair approaches antipodal, where the sine of the half
/// angle flattens against 1. That regime is documented rather than handled: this search asks for
/// distances against a circle radius far below half the globe, so it never reads an answer from it.
#[must_use]
pub fn angular_distance_rad(from: LatLon, to: LatLon) -> f64 {
    let lat_from = from.lat.to_radians();
    let lat_to = to.lat.to_radians();
    let half_delta_lat = (lat_to - lat_from) / 2.0;
    let half_delta_lon = (to.lon - from.lon).to_radians() / 2.0;

    let h =
        half_delta_lat.sin().powi(2) + lat_from.cos() * lat_to.cos() * half_delta_lon.sin().powi(2);
    // Pole to pole puts h a few ulps above 1 — cos(90°) is 6.1e-17 rather than 0, so the second
    // term never quite vanishes. Unclamped that makes sqrt(1 - h) a NaN out of a well-defined
    // input, so the clamp is correctness rather than tidiness.
    let h = h.clamp(0.0, 1.0);

    2.0 * f64::atan2(h.sqrt(), (1.0 - h).sqrt())
}

/// Great-circle distance in kilometres on the [`EARTH_RADIUS_KM`] sphere.
#[must_use]
pub fn great_circle_km(from: LatLon, to: LatLon) -> f64 {
    EARTH_RADIUS_KM * angular_distance_rad(from, to)
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this
// narrow exemption; docs/ai/code.md allows both in tests. float_cmp covers the symmetry assertion,
// where exact equality is the property: the formula is symmetric in its arguments, so any
// asymmetric rewrite of it should fail rather than pass within a tolerance.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
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

    fn at(lat: f64, lon: f64) -> LatLon {
        LatLon { lat, lon }
    }

    fn assert_rel(actual: f64, expected: f64, what: &str) {
        let scale = if expected == 0.0 { 1.0 } else { expected.abs() };
        assert!(
            (actual - expected).abs() / scale < 1e-9,
            "{what}: got {actual}, expected {expected}"
        );
    }

    #[test]
    fn identical_points_are_zero_apart() {
        for p in [
            at(0.0, 0.0),
            at(51.5, -0.12),
            at(90.0, 0.0),
            at(-90.0, 73.0),
        ] {
            assert_eq!(angular_distance_rad(p, p), 0.0);
        }
    }

    #[test]
    fn one_degree_along_the_equator_is_one_degree_of_arc() {
        assert_rel(
            great_circle_km(at(0.0, 0.0), at(0.0, 1.0)),
            EARTH_RADIUS_KM * std::f64::consts::PI / 180.0,
            "one degree of equator",
        );
    }

    #[test]
    fn a_quarter_turn_is_a_quarter_of_the_circumference() {
        let quarter = EARTH_RADIUS_KM * std::f64::consts::FRAC_PI_2;
        assert_rel(
            great_circle_km(at(0.0, 0.0), at(0.0, 90.0)),
            quarter,
            "equator to 90E",
        );
        assert_rel(
            great_circle_km(at(0.0, 0.0), at(90.0, 0.0)),
            quarter,
            "equator to north pole",
        );
    }

    #[test]
    fn antipodes_are_half_the_circumference() {
        let half = EARTH_RADIUS_KM * std::f64::consts::PI;
        assert_rel(
            great_circle_km(at(0.0, 0.0), at(0.0, 180.0)),
            half,
            "east around",
        );
        assert_rel(
            great_circle_km(at(0.0, 0.0), at(0.0, -180.0)),
            half,
            "west around",
        );
        // The case the clamp exists for: unclamped this is a NaN, not a slightly wrong number.
        assert_rel(
            great_circle_km(at(90.0, 0.0), at(-90.0, 0.0)),
            half,
            "pole to pole",
        );
    }

    // Published worked examples, each stating its own earth model. The assertion is on the central
    // angle because that is radius-independent: every source uses a different sphere from ours
    // (6371.2, 6371, and one implied by the nautical mile), and comparing angles removes that
    // disagreement rather than hiding it inside a tolerance. Each tolerance is the precision the
    // source printed — five or six significant figures — not a number picked to pass.
    //
    // 1. White House to Eiffel Tower — "Haversine formula", Wikipedia. Sphere of 6371.2 km.
    // 2. Valparaiso to Shanghai — "Great-circle navigation", Wikipedia. Sphere of 6371 km. This
    //    one crosses the antimeridian and is 168.56 deg long, so it also covers the long-line case.
    // 3. LAX to JFK — Ed Williams, "Aviation Formulary" v1.47, https://edwilliams.org/avform147.htm
    //    Spherical, published directly in radians.
    #[test]
    fn published_central_angles_agree() {
        let cases = [
            (
                "White House to Eiffel Tower",
                at(38.898, -77.037),
                at(48.858, 2.294),
                55.411_f64.to_radians(),
                1e-5,
            ),
            (
                "Valparaiso to Shanghai",
                at(-33.0, -71.6),
                at(31.4, 121.8),
                168.56_f64.to_radians(),
                5e-5,
            ),
            (
                "LAX to JFK",
                at(33.0 + 57.0 / 60.0, -(118.0 + 24.0 / 60.0)),
                at(40.0 + 38.0 / 60.0, -(73.0 + 47.0 / 60.0)),
                0.623_585,
                2e-6,
            ),
        ];

        for (what, from, to, expected_rad, tolerance) in cases {
            let actual = angular_distance_rad(from, to);
            let relative = (actual - expected_rad).abs() / expected_rad;
            assert!(
                relative < tolerance,
                "{what}: {actual} rad against published {expected_rad} rad, off by {relative}"
            );
        }
    }

    // The counterpart check, and the one whose tolerance has to be argued: these are ellipsoidal
    // figures from the same two sources, and a sphere disagrees with WGS 84 by up to about 0.5%
    // (both sources say so). So 0.5% is what they get. Nothing tighter would be meaningful, and the
    // test's job is catching a unit slip, a lat/lon swap or a wrong radius — each of which misses by
    // far more than half a percent — rather than pinning accuracy we do not claim.
    #[test]
    fn published_ellipsoidal_distances_agree_to_half_a_percent() {
        let cases = [
            (
                "White House to Eiffel Tower",
                at(38.898, -77.037),
                at(48.858, 2.294),
                6177.45,
            ),
            (
                "Valparaiso to Shanghai",
                at(-33.0, -71.6),
                at(31.4, 121.8),
                18752.0,
            ),
        ];

        for (what, from, to, ellipsoidal_km) in cases {
            let actual = great_circle_km(from, to);
            let relative = (actual - ellipsoidal_km).abs() / ellipsoidal_km;
            assert!(
                relative < 0.005,
                "{what}: {actual} km against WGS 84's {ellipsoidal_km} km, off by {relative}"
            );
        }
    }

    #[test]
    fn pole_to_pole_is_finite_from_every_meridian() {
        for lon in [-180.0, -73.0, 0.0, 73.0, 179.9] {
            let d = great_circle_km(at(90.0, 0.0), at(-90.0, lon));
            assert!(d.is_finite(), "meridian {lon} produced {d}");
        }
    }

    proptest! {
        #[test]
        fn distance_is_symmetric(
            lat_a in -90.0f64..=90.0,
            lon_a in -180.0f64..180.0,
            lat_b in -90.0f64..=90.0,
            lon_b in -180.0f64..180.0,
        ) {
            let a = at(lat_a, lon_a);
            let b = at(lat_b, lon_b);
            prop_assert_eq!(angular_distance_rad(a, b), angular_distance_rad(b, a));
        }

        #[test]
        fn distance_obeys_the_triangle_inequality(
            lat_a in -90.0f64..=90.0,
            lon_a in -180.0f64..180.0,
            lat_b in -90.0f64..=90.0,
            lon_b in -180.0f64..180.0,
            lat_c in -90.0f64..=90.0,
            lon_c in -180.0f64..180.0,
        ) {
            let a = at(lat_a, lon_a);
            let b = at(lat_b, lon_b);
            let c = at(lat_c, lon_c);
            let direct = angular_distance_rad(a, c);
            let via_b = angular_distance_rad(a, b) + angular_distance_rad(b, c);
            prop_assert!(direct <= via_b + 1e-9, "{} > {}", direct, via_b);
        }

        #[test]
        fn no_pair_of_points_is_further_than_half_the_globe(
            lat_a in -90.0f64..=90.0,
            lon_a in -180.0f64..180.0,
            lat_b in -90.0f64..=90.0,
            lon_b in -180.0f64..180.0,
        ) {
            let d = angular_distance_rad(at(lat_a, lon_a), at(lat_b, lon_b));
            prop_assert!(d.is_finite() && (0.0..=std::f64::consts::PI).contains(&d), "{}", d);
        }

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
