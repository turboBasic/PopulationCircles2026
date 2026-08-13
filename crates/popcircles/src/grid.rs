use std::error::Error;
use std::fmt;

use crate::geodesy::{EARTH_RADIUS_KM, LatLon};

// A step arrives as a decimal from a geotransform and need not round-trip to the rational it
// means, so a span computed from one lands a few ulps off the whole number it is meant to hit —
// past -90 for a grid that in fact ends at the pole, short of 360 for one that in fact closes on
// itself. The size is what keeps this from swallowing a real discrepancy: being out by one cell
// costs 1/120° on the finest grid here, eight orders of magnitude above this.
const BOUNDARY_TOLERANCE_DEG: f64 = 1e-9;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridError {
    EmptyDimension { width: u32, height: u32 },
    NonFiniteOrigin { origin: LatLon },
    OriginLatOutOfRange { origin_lat: f64 },
    NonFiniteStep { lon_step: f64, lat_step: f64 },
    ZeroStep { lon_step: f64, lat_step: f64 },
    LatStepNotSouthward { lat_step: f64 },
    RunsPastSouthPole { south_edge: f64 },
}

impl fmt::Display for GridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::EmptyDimension { width, height } => {
                write!(f, "grid has an empty dimension: {width} x {height}")
            }
            Self::NonFiniteOrigin { origin } => {
                write!(
                    f,
                    "grid origin is not finite: ({}, {})",
                    origin.lat, origin.lon
                )
            }
            Self::OriginLatOutOfRange { origin_lat } => {
                write!(f, "grid origin latitude {origin_lat} is outside [-90, 90]")
            }
            Self::NonFiniteStep { lon_step, lat_step } => {
                write!(f, "grid step is not finite: ({lon_step}, {lat_step})")
            }
            Self::ZeroStep { lon_step, lat_step } => {
                write!(f, "grid step is zero: ({lon_step}, {lat_step})")
            }
            Self::LatStepNotSouthward { lat_step } => write!(
                f,
                "grid lat_step {lat_step} is not negative; rows run north to south"
            ),
            Self::RunsPastSouthPole { south_edge } => {
                write!(f, "grid rows reach {south_edge}, past the south pole")
            }
        }
    }
}

impl Error for GridError {}

/// Named fields rather than a tuple because 3.1's zone formula subtracts these two in one
/// direction only, and a silent swap there flips a sign rather than failing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatBand {
    pub north: f64,
    pub south: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LonSpan {
    pub west: f64,
    pub east: f64,
}

/// Rows run north to south, so `lat_step` is negative — the GDAL north-up convention, kept so a
/// geotransform read from a raster needs no sign flip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    width: u32,
    height: u32,
    origin: LatLon,
    lon_step: f64,
    lat_step: f64,
}

impl Grid {
    /// # Errors
    /// [`GridError`], whose variants name the cases, when the arguments cannot describe a north-up
    /// grid that stays on the globe.
    pub fn new(
        width: u32,
        height: u32,
        origin: LatLon,
        lon_step: f64,
        lat_step: f64,
    ) -> Result<Self, GridError> {
        if width == 0 || height == 0 {
            return Err(GridError::EmptyDimension { width, height });
        }
        if !origin.lat.is_finite() || !origin.lon.is_finite() {
            return Err(GridError::NonFiniteOrigin { origin });
        }
        if !(-90.0..=90.0).contains(&origin.lat) {
            return Err(GridError::OriginLatOutOfRange {
                origin_lat: origin.lat,
            });
        }
        if !lon_step.is_finite() || !lat_step.is_finite() {
            return Err(GridError::NonFiniteStep { lon_step, lat_step });
        }
        if lon_step == 0.0 || lat_step == 0.0 {
            return Err(GridError::ZeroStep { lon_step, lat_step });
        }
        if lat_step > 0.0 {
            return Err(GridError::LatStepNotSouthward { lat_step });
        }

        // u32 -> f64 is exact: f64 represents every integer below 2^53.
        let south_edge = origin.lat + f64::from(height) * lat_step;
        if south_edge < -90.0 - BOUNDARY_TOLERANCE_DEG {
            return Err(GridError::RunsPastSouthPole { south_edge });
        }

        Ok(Self {
            width,
            height,
            origin,
            lon_step,
            lat_step,
        })
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub const fn origin(&self) -> LatLon {
        self.origin
    }

    #[must_use]
    pub const fn lon_step(&self) -> f64 {
        self.lon_step
    }

    #[must_use]
    pub const fn lat_step(&self) -> f64 {
        self.lat_step
    }

    /// The centre of a cell, not its corner: an index addresses a sample, and every distance in the
    /// search is measured between sample centres.
    #[must_use]
    pub fn centre_of(&self, row: u32, col: u32) -> LatLon {
        // u32 -> f64 is exact below 2^53, so these carry no rounding of their own.
        LatLon {
            lat: self.origin.lat + (f64::from(row) + 0.5) * self.lat_step,
            lon: self.origin.lon + (f64::from(col) + 0.5) * self.lon_step,
        }
    }

    #[must_use]
    pub fn lat_bounds(&self, row: u32) -> LatBand {
        let north = self.origin.lat + f64::from(row) * self.lat_step;
        // lat_step is negative by construction, so north is the larger of the two.
        LatBand {
            north,
            south: north + self.lat_step,
        }
    }

    #[must_use]
    pub fn lon_bounds(&self, col: u32) -> LonSpan {
        let a = self.origin.lon + f64::from(col) * self.lon_step;
        let b = a + self.lon_step;
        // lon_step's sign is not constrained, so order by value rather than by index.
        LonSpan {
            west: a.min(b),
            east: a.max(b),
        }
    }

    /// The cell containing a coordinate, or `None` when it falls outside the grid.
    ///
    /// Longitude is reduced modulo a full turn first, so on a whole-globe grid every longitude
    /// lands in a column and the antimeridian is not a boundary. On a window grid a longitude
    /// outside the window has no column, and that is the `None`.
    #[must_use]
    pub fn cell_containing(&self, at: LatLon) -> Option<(u32, u32)> {
        if !at.lat.is_finite() || !at.lon.is_finite() {
            return None;
        }

        let row = index_from_offset((at.lat - self.origin.lat) / self.lat_step, self.height)?;

        let cells_from_origin = (at.lon - self.origin.lon) / self.lon_step;
        // Reducing by the width rather than by 360/step is what makes "every longitude has a
        // column" exact on a closed grid: 360/step can exceed the width by a rounding sliver, and
        // longitudes falling in that sliver would otherwise have no column at all.
        let modulus = if self.spans_full_turn() {
            f64::from(self.width)
        } else {
            360.0 / self.lon_step.abs()
        };
        let col = index_from_offset(cells_from_origin.rem_euclid(modulus), self.width)?;

        Some((row, col))
    }

    /// The ground area of any cell in a row, by the spherical zone formula
    /// `R² · Δλ · (sin φ_north − sin φ_south)`. Every cell in a row has the same area, so this takes
    /// no column.
    ///
    /// It reads the row's edges through [`Grid::lat_bounds`] rather than recomputing them, which is
    /// what stops the area and the coordinate of a row from ever disagreeing about where the row is.
    #[must_use]
    pub fn cell_area_km2(&self, row: u32) -> f64 {
        let band = self.lat_bounds(row);
        let delta_lon_rad = self.lon_step.abs().to_radians();
        EARTH_RADIUS_KM
            * EARTH_RADIUS_KM
            * delta_lon_rad
            * (band.north.to_radians().sin() - band.south.to_radians().sin())
    }

    /// Whether the columns close on themselves, as a whole-globe raster's do.
    #[must_use]
    pub fn spans_full_turn(&self) -> bool {
        (f64::from(self.width) * self.lon_step.abs() - 360.0).abs() <= BOUNDARY_TOLERANCE_DEG
    }
}

/// `offset` is a position in cells from the origin along one axis; the index is the cell it falls
/// in, or `None` when that is off the end.
fn index_from_offset(offset: f64, limit: u32) -> Option<u32> {
    let floored = offset.floor();
    if !(0.0..f64::from(limit)).contains(&floored) {
        return None;
    }
    // The range check above proves floored is an integral f64 in [0, limit) and limit is a u32, so
    // this conversion is exact. std offers no fallible f64 -> u32, which is why it is a cast.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some(floored as u32)
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this
// narrow exemption; docs/ai/code.md allows both in tests. float_cmp is here because these
// assertions pin that the constructor stored its arguments verbatim — bit-exact equality is the
// property, not an approximation of one.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::geodesy::wrap_lon;

    const GPW_STEP: f64 = 1.0 / 120.0;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    // The decimated grid application.md "Approach" calls for: same code path, coarse enough to
    // enumerate exhaustively in a test.
    fn decimated() -> Grid {
        Grid::new(
            360,
            180,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            1.0,
            -1.0,
        )
        .expect("a 1 degree whole-globe grid is valid")
    }

    fn gpw() -> Grid {
        Grid::new(
            43200,
            21600,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            GPW_STEP,
            -GPW_STEP,
        )
        .expect("the GPW registry row is a valid grid")
    }

    #[test]
    fn gpw_registry_row_constructs() {
        let grid = gpw();
        assert_eq!(grid.width(), 43200);
        assert_eq!(grid.height(), 21600);
        assert_eq!(grid.origin().lat, 90.0);
        assert_eq!(grid.origin().lon, -180.0);
        assert_eq!(grid.lat_step(), -GPW_STEP);
    }

    fn err(width: u32, height: u32, origin_lat: f64, lon_step: f64, lat_step: f64) -> GridError {
        Grid::new(
            width,
            height,
            LatLon {
                lat: origin_lat,
                lon: -180.0,
            },
            lon_step,
            lat_step,
        )
        .expect_err("expected this grid to be rejected")
    }

    #[test]
    fn rejects_empty_dimension() {
        assert_eq!(
            err(0, 21600, 90.0, GPW_STEP, -GPW_STEP),
            GridError::EmptyDimension {
                width: 0,
                height: 21600
            }
        );
        assert_eq!(
            err(43200, 0, 90.0, GPW_STEP, -GPW_STEP),
            GridError::EmptyDimension {
                width: 43200,
                height: 0
            }
        );
    }

    #[test]
    fn rejects_non_finite_origin() {
        assert!(matches!(
            err(43200, 21600, f64::NAN, GPW_STEP, -GPW_STEP),
            GridError::NonFiniteOrigin { .. }
        ));
        assert!(matches!(
            Grid::new(
                43200,
                21600,
                LatLon {
                    lat: 90.0,
                    lon: f64::INFINITY
                },
                GPW_STEP,
                -GPW_STEP
            ),
            Err(GridError::NonFiniteOrigin { .. })
        ));
    }

    #[test]
    fn rejects_origin_lat_out_of_range() {
        assert_eq!(
            err(43200, 21600, 90.5, GPW_STEP, -GPW_STEP),
            GridError::OriginLatOutOfRange { origin_lat: 90.5 }
        );
        assert_eq!(
            err(43200, 21600, -90.5, GPW_STEP, -GPW_STEP),
            GridError::OriginLatOutOfRange { origin_lat: -90.5 }
        );
    }

    #[test]
    fn rejects_non_finite_step() {
        assert!(matches!(
            err(43200, 21600, 90.0, f64::NAN, -GPW_STEP),
            GridError::NonFiniteStep { .. }
        ));
        assert!(matches!(
            err(43200, 21600, 90.0, GPW_STEP, f64::NEG_INFINITY),
            GridError::NonFiniteStep { .. }
        ));
    }

    #[test]
    fn rejects_zero_step() {
        assert!(matches!(
            err(43200, 21600, 90.0, 0.0, -GPW_STEP),
            GridError::ZeroStep { .. }
        ));
        assert!(matches!(
            err(43200, 21600, 90.0, GPW_STEP, 0.0),
            GridError::ZeroStep { .. }
        ));
    }

    #[test]
    fn rejects_northward_lat_step() {
        assert_eq!(
            err(43200, 21600, -90.0, GPW_STEP, GPW_STEP),
            GridError::LatStepNotSouthward { lat_step: GPW_STEP }
        );
    }

    #[test]
    fn rejects_rows_past_the_south_pole() {
        // One row too many at the GPW step overruns by a whole cell, well past the rounding the
        // tolerance admits.
        assert!(matches!(
            err(43200, 21601, 90.0, GPW_STEP, -GPW_STEP),
            GridError::RunsPastSouthPole { .. }
        ));
    }

    #[test]
    fn a_step_a_hair_too_large_still_reaches_the_pole() {
        // What POLE_TOLERANCE_DEG is for: this grid ends at -90 in intent but computes a south edge
        // just past it. Rejecting it would fail a real raster on a rounding artefact.
        let step = f64::from_bits((GPW_STEP).to_bits() + 1);
        assert!(90.0 + f64::from(21600u32) * -step < -90.0);
        Grid::new(
            43200,
            21600,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            step,
            -step,
        )
        .expect("a step one ulp large is rounding, not an overrun");
    }

    #[test]
    fn a_partial_grid_north_of_the_pole_constructs() {
        // A country mask is a window on the globe, not the whole globe: rows stopping short of -90
        // are the normal case, not an edge case.
        Grid::new(
            1200,
            600,
            LatLon {
                lat: 60.0,
                lon: -10.0,
            },
            GPW_STEP,
            -GPW_STEP,
        )
        .expect("a window grid inside the globe is valid");
    }

    #[test]
    fn centres_sit_half_a_cell_inside_each_corner() {
        let grid = gpw();
        let half = GPW_STEP / 2.0;

        let nw = grid.centre_of(0, 0);
        assert!(close(nw.lat, 90.0 - half) && close(nw.lon, -180.0 + half));

        let ne = grid.centre_of(0, 43199);
        assert!(close(ne.lat, 90.0 - half) && close(ne.lon, 180.0 - half));

        let sw = grid.centre_of(21599, 0);
        assert!(close(sw.lat, -90.0 + half) && close(sw.lon, -180.0 + half));

        let se = grid.centre_of(21599, 43199);
        assert!(close(se.lat, -90.0 + half) && close(se.lon, 180.0 - half));
    }

    #[test]
    fn polar_rows_stay_on_the_globe() {
        let grid = gpw();
        for row in [0, grid.height() - 1] {
            let centre = grid.centre_of(row, 0);
            assert!(centre.lat.abs() < 90.0, "row {row} centre left the globe");
            assert_eq!(grid.cell_containing(centre), Some((row, 0)));
        }
    }

    #[test]
    fn the_antimeridian_is_not_a_boundary() {
        let grid = gpw();
        let last = grid.width() - 1;

        // The two columns either side of the seam are the first and the last, and a longitude in
        // each lands in it — the seam is where the index wraps, not where the grid stops.
        assert_eq!(
            grid.cell_containing(grid.centre_of(0, last)),
            Some((0, last))
        );
        assert_eq!(grid.cell_containing(grid.centre_of(0, 0)), Some((0, 0)));

        // -180 and +180 are the same meridian, so both land in a column, and +180 wraps to the
        // first rather than falling off the last.
        assert_eq!(
            grid.cell_containing(LatLon {
                lat: 0.0,
                lon: -180.0
            }),
            Some((10800, 0))
        );
        assert_eq!(
            grid.cell_containing(LatLon {
                lat: 0.0,
                lon: 180.0
            }),
            Some((10800, 0))
        );
    }

    #[test]
    fn bounds_bracket_the_centre() {
        let grid = gpw();
        for (row, col) in [(0u32, 0u32), (10800, 21600), (21599, 43199)] {
            let centre = grid.centre_of(row, col);
            let band = grid.lat_bounds(row);
            let span = grid.lon_bounds(col);
            assert!(band.north > centre.lat && centre.lat > band.south);
            assert!(span.west < centre.lon && centre.lon < span.east);
            assert!(close(band.north - band.south, GPW_STEP));
            assert!(close(span.east - span.west, GPW_STEP));
        }
    }

    #[test]
    fn adjacent_rows_share_an_edge() {
        // The bands tile the globe with no gap and no overlap, which is what lets 3.1's sum
        // telescope.
        let grid = decimated();
        for row in 0..grid.height() - 1 {
            assert!(close(
                grid.lat_bounds(row).south,
                grid.lat_bounds(row + 1).north
            ));
        }
        assert!(close(grid.lat_bounds(0).north, 90.0));
        assert!(close(grid.lat_bounds(grid.height() - 1).south, -90.0));
    }

    #[test]
    fn a_latitude_off_the_grid_has_no_cell() {
        let grid = gpw();
        for lat in [90.5, -90.5, f64::NAN, f64::INFINITY] {
            assert_eq!(grid.cell_containing(LatLon { lat, lon: 0.0 }), None);
        }
    }

    #[test]
    fn a_latitude_past_a_pole_is_rejected_not_folded() {
        // The failure this rules out: 91 folding to 89 and quietly answering with a cell on the
        // wrong side of the pole. Rejection is the whole point, so assert it is not the fold.
        let grid = gpw();
        let folded = grid.cell_containing(LatLon {
            lat: 89.0,
            lon: 0.0,
        });
        assert!(folded.is_some());
        assert_eq!(
            grid.cell_containing(LatLon {
                lat: 91.0,
                lon: 0.0
            }),
            None
        );
        assert_eq!(
            grid.cell_containing(LatLon {
                lat: -91.0,
                lon: 0.0
            }),
            None
        );
    }

    #[test]
    fn a_grid_whose_origin_is_a_whole_turn_away_is_the_same_grid() {
        let base = gpw();
        for k in -3..=3 {
            let shifted = Grid::new(
                43200,
                21600,
                LatLon {
                    lat: 90.0,
                    lon: -180.0 + 360.0 * f64::from(k),
                },
                GPW_STEP,
                -GPW_STEP,
            )
            .expect("shifting the origin by whole turns keeps the grid valid");
            for (row, col) in [(0u32, 0u32), (10800, 21600), (21599, 43199)] {
                let here = shifted.centre_of(row, col);
                let there = base.centre_of(row, col);
                assert!(close(here.lat, there.lat));
                assert!(
                    (wrap_lon(here.lon) - wrap_lon(there.lon)).abs() < 1e-9,
                    "k={k} ({row}, {col})"
                );
            }
        }
    }

    #[test]
    fn a_longitude_outside_a_window_grid_has_no_cell() {
        // The counterpart of "every longitude has a column": that holds because the globe closes,
        // and a window does not close, so its outside is a real None.
        let window = Grid::new(
            600,
            600,
            LatLon {
                lat: 60.0,
                lon: -10.0,
            },
            GPW_STEP,
            -GPW_STEP,
        )
        .expect("a window grid is valid");
        assert!(!window.spans_full_turn());
        assert_eq!(
            window.cell_containing(LatLon {
                lat: 57.0,
                lon: 120.0
            }),
            None
        );
        assert!(
            window
                .cell_containing(LatLon {
                    lat: 57.0,
                    lon: -7.0
                })
                .is_some()
        );
    }

    #[test]
    fn whole_globe_cell_areas_sum_to_the_sphere() {
        // The zone formula telescopes: summing a whole globe leaves sin(90) - sin(-90) = 2, so this
        // is an identity rather than an approximation, and a loose tolerance here would hide a real
        // disagreement rather than absorb rounding. Naive summation over 21600 rows costs about
        // 5e-14 relative, four orders inside the bound.
        let exact = 4.0 * std::f64::consts::PI * EARTH_RADIUS_KM * EARTH_RADIUS_KM;
        for grid in [gpw(), decimated()] {
            let total: f64 = (0..grid.height())
                .map(|row| f64::from(grid.width()) * grid.cell_area_km2(row))
                .sum();
            let relative = (total - exact).abs() / exact;
            assert!(
                relative < 1e-9,
                "{} x {}: {total} km2 against 4piR^2 = {exact} km2, off by {relative}",
                grid.width(),
                grid.height()
            );
        }
    }

    #[test]
    fn cell_areas_are_positive_and_symmetric_about_the_equator() {
        let grid = decimated();
        let last = grid.height() - 1;
        for row in 0..grid.height() {
            let area = grid.cell_area_km2(row);
            assert!(area > 0.0, "row {row} has area {area}");
            let mirrored = grid.cell_area_km2(last - row);
            assert!(
                (area - mirrored).abs() / area < 1e-12,
                "row {row} and its mirror disagree: {area} against {mirrored}"
            );
        }
    }

    #[test]
    fn cell_areas_grow_towards_the_equator() {
        // A cell spans a fixed angle, so its ground width shrinks with the cosine of latitude. The
        // largest cells are the two straddling the equator, and nothing on the way there dips.
        for grid in [gpw(), decimated()] {
            let middle = grid.height() / 2;
            for row in 1..middle {
                let previous = grid.cell_area_km2(row - 1);
                let current = grid.cell_area_km2(row);
                assert!(
                    current > previous,
                    "row {row} ({current}) is not larger than row {} ({previous})",
                    row - 1
                );
            }
            let largest = (0..grid.height())
                .map(|row| grid.cell_area_km2(row))
                .fold(f64::NEG_INFINITY, f64::max);
            assert!((grid.cell_area_km2(middle) - largest).abs() / largest < 1e-12);
        }
    }

    proptest! {
        #[test]
        fn centre_round_trips_on_the_full_grid(row in 0u32..21600, col in 0u32..43200) {
            let grid = gpw();
            prop_assert_eq!(grid.cell_containing(grid.centre_of(row, col)), Some((row, col)));
        }

        #[test]
        fn centre_round_trips_on_the_decimated_grid(row in 0u32..180, col in 0u32..360) {
            let grid = decimated();
            prop_assert_eq!(grid.cell_containing(grid.centre_of(row, col)), Some((row, col)));
        }

        #[test]
        #[test]
        fn a_whole_turn_of_longitude_finds_the_same_cell(
            row in 0u32..21600,
            col in 0u32..43200,
            k in -3i32..=3,
        ) {
            let grid = gpw();
            let centre = grid.centre_of(row, col);
            let shifted = LatLon { lat: centre.lat, lon: centre.lon + 360.0 * f64::from(k) };
            prop_assert_eq!(grid.cell_containing(shifted), Some((row, col)));
        }

        #[test]
        fn every_longitude_has_a_column(lon in -3600.0f64..3600.0) {
            let grid = gpw();
            // Bound out of the assert: prop_assert! stringifies its expression into a format
            // string, so a struct literal's braces break the macro.
            let at = LatLon { lat: 0.0, lon };
            prop_assert!(grid.cell_containing(at).is_some());
        }
    }
}
