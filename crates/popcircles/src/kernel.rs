// Step 2 of application.md "Approach": the columns a spherical cap of a given ground radius covers in
// each row of a grid. A span is an offset from a centre column rather than a pair of column indices,
// which is what makes one kernel serve every centre on its row and leaves the seam to one place.
//
// A cell is in the cap when its centre is within the cap's angular radius of the cap centre, measured
// through geodesy. That is the definition, and the spherical law of cosines below is an estimate the
// definition corrects rather than the answer: arccos loses precision exactly at the cap boundary, where
// its derivative is unbounded, and what the search needs is a membership rule a brute-force distance
// test reproduces — application.md "Correctness invariants".

use crate::geodesy::{LatLon, angular_distance_rad, central_angle_rad};
use crate::grid::{Grid, Row};

#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum KernelError {
    #[error(
        "a kernel is longitude-invariant only where the columns close; these span {lon_span} degrees"
    )]
    ColumnsDoNotClose { lon_span: f64 },

    #[error("a circle radius must be finite; {radius_km} km is not")]
    RadiusNotFinite { radius_km: f64 },

    #[error("a circle radius must not be negative; {radius_km} km is")]
    RadiusNegative { radius_km: f64 },
}

/// The columns one row of a kernel covers, as an offset from the centre column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    /// `half_width` columns either side of the centre column, and the centre column itself: `2h + 1`
    /// columns in all.
    Around { half_width: u32 },
    /// Every column of the row. A variant rather than the half width that would reach it, for
    /// [`crate::table::ColSpan::FullTurn`]'s reason: a row wide enough to close on itself must not be
    /// assembled from two pieces that double-count where they meet.
    FullTurn,
}

/// The cap being measured, in the terms one row's span needs, and the only place the geometry lives.
#[derive(Debug, Clone, Copy)]
struct Cap {
    angle_rad: f64,
    centre_lat: f64,
    lon_step: f64,
    width: u32,
}

impl Cap {
    /// The radius becomes an angle once, here, and is clamped at half a turn: no two points on a sphere
    /// are further apart than that, and an angle past π would leave `cos θ` in [`Cap::estimate`]
    /// oscillating instead of saturating — an estimate the walk would then pay for a column at a time.
    fn over(grid: &Grid, centre_lat: f64, radius_km: f64) -> Self {
        Self {
            angle_rad: central_angle_rad(radius_km).min(std::f64::consts::PI),
            centre_lat,
            lon_step: grid.lon_step(),
            width: grid.width(),
        }
    }

    /// The smallest half width that closes a row: `2h + 1 >= width` holds here at both parities, so
    /// reaching it is the full turn rather than a span a column short of one.
    const fn closing(&self) -> u32 {
        self.width / 2
    }

    /// Whether the cell `cells` columns along the row at `lat` is in the cap.
    ///
    /// The longitude is an **offset** — `cells` steps of the grid's own step — and never the difference
    /// of two [`Grid::centre_of`] longitudes, which is a whole multiple of that step only to within a
    /// rounding. Taking the offset is what makes a kernel exactly longitude-invariant rather than
    /// invariant to within the rounding that decides a boundary cell.
    fn contains(&self, lat: f64, cells: u32) -> bool {
        let centre = LatLon {
            lat: self.centre_lat,
            lon: 0.0,
        };
        let along = LatLon {
            lat,
            lon: f64::from(cells) * self.lon_step,
        };
        angular_distance_rad(centre, along) <= self.angle_rad
    }

    /// Whether the cap reaches the row at `lat` at all.
    ///
    /// The cell on the cap's own meridian is the row's closest to the centre, because for two fixed
    /// latitudes the great-circle distance grows with the longitude difference across the half turn. So
    /// a row this refuses holds no cell of the cap, and the rows it admits are contiguous.
    fn reaches(&self, lat: f64) -> bool {
        self.contains(lat, 0)
    }

    /// The columns the cap covers in a row it reaches.
    fn span(&self, lat: f64) -> Span {
        let closing = self.closing();
        let mut half_width = self.estimate(lat).min(closing);
        // The estimate is a starting point and `contains` is the definition, so this walk is what makes
        // the two agree. Either loop moves a step or two in practice, and no estimate can make the
        // answer wrong — only slow. What makes them terminate rather than merely stop is the same
        // monotonicity `reaches` rests on.
        while half_width < closing && self.contains(lat, half_width + 1) {
            half_width += 1;
        }
        while half_width > 0 && !self.contains(lat, half_width) {
            half_width -= 1;
        }

        if half_width == closing {
            Span::FullTurn
        } else {
            Span::Around { half_width }
        }
    }

    /// The half width the spherical law of cosines gives: `cos θ = sin φ₀ sin φ + cos φ₀ cos φ cos Δλ`,
    /// solved for Δλ and floored to whole columns.
    fn estimate(&self, lat: f64) -> u32 {
        let closing = self.closing();
        let (centre, row) = (self.centre_lat.to_radians(), lat.to_radians());
        let ratio = (self.angle_rad.cos() - centre.sin() * row.sin()) / (centre.cos() * row.cos());

        // The two degenerate branches, explicit because no arccos answers either: at or past 1 the cap
        // reaches no further than its own meridian in this row, and at or past −1 it covers the row.
        if ratio >= 1.0 {
            return 0;
        }
        if ratio <= -1.0 {
            return closing;
        }

        let cells = ratio.acos().to_degrees() / self.lon_step.abs();
        // Also where a ratio that is not a number lands, having taken neither branch above. Nothing
        // reachable through [`Kernel`] produces one — a grid's latitudes are finite and the cosine of a
        // representable latitude is never exactly zero, `cos(90°)` being 6.1e-17, so the denominator
        // does not collapse — and this is about what such an input would cost rather than what it
        // returns, since the walk in `span` corrects any estimate.
        if cells.is_nan() || cells >= f64::from(closing) {
            return closing;
        }
        // `cells` is finite and inside `[0, closing)`, so truncating is exact, and truncation is the
        // floor this wants: the last column the estimate claims. std offers no fallible f64 -> u32.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        (cells as u32)
    }
}

/// A cap of a given ground radius, decomposed into the columns it covers in each row it reaches.
///
/// It is built for a **row**, not for a cell: the shape depends on the centre's latitude and on nothing
/// else, so one kernel serves every centre on that row and the longitudes enter only when it is placed.
///
/// A grid whose columns do not close has no kernel. Invariance is the whole point of the type, and on a
/// grid that does not close a span would need clipping that depends on which column the centre is —
/// which is to say it would not be one shape but a different shape per centre.
#[derive(Debug, Clone, PartialEq)]
pub struct Kernel {
    grid: Grid,
    centre: Row,
    radius_km: f64,
    north: Row,
    spans: Vec<Span>,
}

impl Kernel {
    /// # Errors
    /// [`KernelError::ColumnsDoNotClose`] when the grid's columns do not close on themselves;
    /// [`KernelError::RadiusNotFinite`] or [`KernelError::RadiusNegative`] when the radius is not a
    /// length. Zero is a length: the cap is the centre cell alone.
    ///
    /// # Panics
    /// If `centre` was minted by a larger grid; [`crate::grid::Row`] says why that is a stop.
    pub fn new(grid: Grid, centre: Row, radius_km: f64) -> Result<Self, KernelError> {
        if !grid.spans_full_turn() {
            return Err(KernelError::ColumnsDoNotClose {
                lon_span: f64::from(grid.width()) * grid.lon_step().abs(),
            });
        }
        if !radius_km.is_finite() {
            return Err(KernelError::RadiusNotFinite { radius_km });
        }
        if radius_km < 0.0 {
            return Err(KernelError::RadiusNegative { radius_km });
        }

        let cap = Cap::over(&grid, grid.centre_lat(centre), radius_km);

        // North then south from the centre row, each stopping at the first row the cap does not reach:
        // the rows it reaches are contiguous, so a first miss is the end of the band and not a gap in
        // it. The grid's own ends stop the walk too, which is how a cap reaching past a pole — or past
        // the edge of a grid covering a band of latitude only — is clipped rather than refused.
        let mut north = centre;
        while let Some(previous) = north.get().checked_sub(1).and_then(|index| grid.row(index)) {
            if !cap.reaches(grid.centre_lat(previous)) {
                break;
            }
            north = previous;
        }
        let mut south = centre;
        while let Some(next) = south.get().checked_add(1).and_then(|index| grid.row(index)) {
            if !cap.reaches(grid.centre_lat(next)) {
                break;
            }
            south = next;
        }

        let spans = grid
            .rows()
            .skip(north.get() as usize)
            .take_while(|row| *row <= south)
            .map(|row| cap.span(grid.centre_lat(row)))
            .collect();

        Ok(Self {
            grid,
            centre,
            radius_km,
            north,
            spans,
        })
    }

    #[must_use]
    pub const fn centre(&self) -> Row {
        self.centre
    }

    #[must_use]
    pub const fn radius_km(&self) -> f64 {
        self.radius_km
    }

    /// Every row the cap reaches, north to south, with the columns it covers there.
    pub fn rows(&self) -> impl Iterator<Item = (Row, Span)> + '_ {
        self.grid
            .rows()
            .skip(self.north.get() as usize)
            .zip(self.spans.iter().copied())
    }
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this narrow
// exemption; docs/ai/code.md allows both in tests. float_cmp is here for the spans compared as values:
// a Span holds an integer count, and comparing two kernels' spans is exact by construction.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use std::collections::BTreeMap;

    use proptest::prelude::*;

    use super::*;
    use crate::geodesy::great_circle_km;

    /// A whole-globe grid, which is the shape that has kernels at all.
    fn globe(cells_per_degree: u32) -> Grid {
        let step = 1.0 / f64::from(cells_per_degree);
        Grid::new(
            360 * cells_per_degree,
            180 * cells_per_degree,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            step,
            -step,
        )
        .expect("a whole-globe grid at a whole number of cells per degree is valid")
    }

    fn row(grid: &Grid, index: u32) -> Row {
        grid.row(index).expect("a row of the fixture")
    }

    fn kernel(grid: Grid, centre: u32, radius_km: f64) -> Kernel {
        Kernel::new(grid, row(&grid, centre), radius_km)
            .expect("a whole-globe grid and a radius that is a length")
    }

    #[test]
    fn a_cap_reaching_no_further_than_its_own_meridian_has_no_width() {
        // The ratio is exactly 1 here: the cap's edge touches this row on the cap's meridian and nowhere
        // else, so there is no arccos to take and the row is one cell wide.
        let cap = Cap {
            angle_rad: 1.0f64.to_radians(),
            centre_lat: 0.0,
            lon_step: 1.0,
            width: 360,
        };
        assert_eq!(cap.estimate(1.0), 0);
        assert_eq!(cap.span(1.0), Span::Around { half_width: 0 });
    }

    #[test]
    fn a_cap_swallowing_a_parallel_covers_the_whole_row() {
        // The other degenerate branch: a cap centred beside the pole and wide enough that every column
        // of this row is inside it, where the ratio is far past −1.
        let cap = Cap {
            angle_rad: 90.0f64.to_radians(),
            centre_lat: 89.0,
            lon_step: 1.0,
            width: 360,
        };
        assert_eq!(cap.estimate(88.0), 180);
        assert_eq!(cap.span(88.0), Span::FullTurn);
    }

    #[test]
    fn an_estimate_that_is_not_a_number_clamps_rather_than_running_away() {
        // Unreachable through `Kernel`, and constructed here directly for that reason. What it pins is
        // the cost of an impossible input rather than its answer: the walk would correct any estimate,
        // and an unbounded one would make it walk half a row to do so.
        let cap = Cap {
            angle_rad: f64::NAN,
            centre_lat: 0.0,
            lon_step: 1.0,
            width: 360,
        };
        assert_eq!(cap.estimate(0.0), 180);
    }

    #[test]
    fn every_rows_span_is_the_columns_a_distance_test_admits() {
        let grid = globe(1);
        for centre_lat in [0.0, 12.5, -47.5, 60.5, 89.5] {
            for radius_km in [250.0, 1000.0, 3000.0, 8000.0] {
                let cap = Cap::over(&grid, centre_lat, radius_km);
                for row in grid.rows() {
                    let lat = grid.centre_lat(row);
                    if !cap.reaches(lat) {
                        continue;
                    }

                    // Kilometres against kilometres, which is the comparison the answer has to be
                    // reproducible in and is independent of the angle the cap converted the radius to.
                    let inside = |cells: u32| {
                        great_circle_km(
                            LatLon {
                                lat: centre_lat,
                                lon: 0.0,
                            },
                            LatLon {
                                lat,
                                lon: f64::from(cells) * grid.lon_step(),
                            },
                        ) <= radius_km
                    };
                    let reach = (0..=cap.closing())
                        .take_while(|cells| inside(*cells))
                        .last()
                        .expect("the cap reaches this row, so its meridian cell is in");
                    let expected = if reach == cap.closing() {
                        Span::FullTurn
                    } else {
                        Span::Around { half_width: reach }
                    };

                    assert_eq!(
                        cap.span(lat),
                        expected,
                        "centre {centre_lat}, radius {radius_km} km, row {}",
                        row.get()
                    );
                }
            }
        }
    }

    #[test]
    fn a_row_that_closes_is_the_full_turn_at_either_parity_of_the_width() {
        // Four columns and five: `closing` is 2 either way, and 2 · 2 + 1 covers five exactly and
        // overruns four. What must not happen is a row reported as `Around` when its span has closed,
        // which is the double count `ColSpan::FullTurn` exists to prevent.
        for width in [4u32, 5] {
            let grid = Grid::new(
                width,
                90,
                LatLon {
                    lat: 90.0,
                    lon: -180.0,
                },
                360.0 / f64::from(width),
                -2.0,
            )
            .expect("a whole-globe grid of any width is valid");

            for radius_km in [100.0, 1000.0, 6000.0, 12_000.0, 20_016.0] {
                let cap = Cap::over(&grid, grid.centre_lat(grid.middle_row()), radius_km);
                for row in grid.rows() {
                    let lat = grid.centre_lat(row);
                    if !cap.reaches(lat) {
                        continue;
                    }
                    match cap.span(lat) {
                        Span::Around { half_width } => {
                            assert!(
                                2 * half_width + 1 < width,
                                "{width} columns, {radius_km} km"
                            );
                            assert!(
                                !cap.contains(lat, half_width + 1),
                                "{width} columns, {radius_km} km: the span stops a column short"
                            );
                        }
                        Span::FullTurn => assert!(
                            cap.contains(lat, cap.closing()),
                            "{width} columns, {radius_km} km: a row closed that the cap does not cover"
                        ),
                    }
                }
            }
        }
    }

    #[test]
    fn a_grid_whose_columns_do_not_close_has_no_kernel() {
        let window = Grid::new(
            600,
            600,
            LatLon {
                lat: 60.0,
                lon: -10.0,
            },
            1.0 / 120.0,
            -1.0 / 120.0,
        )
        .expect("a window grid is valid");
        assert!(matches!(
            Kernel::new(window, row(&window, 0), 500.0),
            Err(KernelError::ColumnsDoNotClose { .. })
        ));
    }

    #[test]
    fn a_radius_that_is_not_a_length_has_no_kernel() {
        let grid = globe(1);
        let centre = grid.middle_row();
        for radius_km in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                matches!(
                    Kernel::new(grid, centre, radius_km),
                    Err(KernelError::RadiusNotFinite { .. })
                ),
                "{radius_km}"
            );
        }
        assert_eq!(
            Kernel::new(grid, centre, -1.0).unwrap_err(),
            KernelError::RadiusNegative { radius_km: -1.0 }
        );
    }

    #[test]
    fn a_zero_radius_is_the_centre_cell_alone() {
        // Degenerate and legal: the centre's own cell is within zero of itself and no other cell is.
        let grid = globe(1);
        let centre = row(&grid, 100);
        let kernel = Kernel::new(grid, centre, 0.0).expect("zero is a length");
        assert_eq!(
            kernel.rows().collect::<Vec<(Row, Span)>>(),
            vec![(centre, Span::Around { half_width: 0 })]
        );
    }

    #[test]
    fn a_cap_over_the_pole_closes_the_rows_beside_it() {
        // Centre 87.5 N, radius 1000 km = 8.993 degrees. The pole is 2.5 degrees away, so a parallel up
        // to 6.49 degrees down the far side is swallowed whole: 84.5 N is 2.5 + 5.5 = 8 degrees off and
        // closes, 83.5 N is 9 and does not. Nothing here takes a branch of its own — the ordinary walk
        // and the ordinary estimate produce it.
        let grid = globe(1);
        let spans: Vec<(Row, Span)> = kernel(grid, 2, 1000.0).rows().collect();

        assert_eq!(spans[0], (row(&grid, 0), Span::FullTurn));
        assert_eq!(spans[5], (row(&grid, 5), Span::FullTurn));
        assert!(matches!(spans[6], (_, Span::Around { .. })));
        // The band ends at 79.5 N, 8 degrees south of the centre, and every row from there north is one
        // the cap reaches.
        assert_eq!(spans.len(), 11);
        assert!(matches!(spans[10], (_, Span::Around { .. })));
    }

    #[test]
    fn a_cap_larger_than_the_globe_closes_every_row() {
        // Half the circumference is 20 015.09 km, so this reaches every point on the sphere. Every row
        // closes as a variant, and none of them as a wrapped pair.
        let grid = globe(1);
        let kernel = kernel(grid, 90, 20_016.0);
        assert_eq!(kernel.rows().count(), grid.height() as usize);
        assert!(kernel.rows().all(|(_, span)| span == Span::FullTurn));
    }

    #[test]
    fn a_cap_running_off_a_band_grid_is_clipped_rather_than_refused() {
        // Closing in longitude, thirty degrees of latitude: the shape a regional grid has. The cap wants
        // to reach 86.5 N and the grid starts at 59.5, so the band's northern end is the grid's own.
        let band = Grid::new(
            360,
            30,
            LatLon {
                lat: 60.0,
                lon: -180.0,
            },
            1.0,
            -1.0,
        )
        .expect("a band grid that closes in longitude is valid");
        let kernel = Kernel::new(band, row(&band, 0), 3000.0).expect("a band grid has kernels");

        let rows: Vec<Row> = kernel.rows().map(|(row, _)| row).collect();
        // 3000 km is 26.979 degrees, so the band runs from 59.5 N to 33.5 N: twenty-seven rows, of which
        // the first is the grid's first and not the cap's.
        assert_eq!(rows.first(), Some(&row(&band, 0)));
        assert_eq!(rows.len(), 27);
    }

    #[test]
    #[should_panic(expected = "is not a row of a 180-row grid")]
    fn a_finer_grids_row_is_no_centre_here() {
        let centre = row(&globe(4), 719);
        let _ = Kernel::new(globe(1), centre, 500.0);
    }

    /// Whether `wider` covers every column `narrower` does, both being spans about one centre.
    fn covers(wider: Span, narrower: Span) -> bool {
        match (wider, narrower) {
            (Span::FullTurn, _) => true,
            (Span::Around { .. }, Span::FullTurn) => false,
            (Span::Around { half_width: wide }, Span::Around { half_width: narrow }) => {
                wide >= narrow
            }
        }
    }

    proptest! {
        /// The property #7's binary search over radius rests on: population is monotone in radius only
        /// if the cells are, and a span that narrowed anywhere as the radius grew would break it while
        /// still returning a plausible number.
        #[test]
        fn growing_the_radius_never_narrows_a_row_or_shortens_the_band(
            centre in 0u32..180,
            radius_km in 50.0f64..9000.0,
            growth in 1.0f64..2000.0,
        ) {
            let grid = globe(1);
            let grown: BTreeMap<Row, Span> = kernel(grid, centre, radius_km + growth).rows().collect();

            for (row, span) in kernel(grid, centre, radius_km).rows() {
                let wider = grown.get(&row).copied();
                prop_assert!(wider.is_some(), "row {} left the band", row.get());
                prop_assert!(
                    covers(wider.unwrap(), span),
                    "row {} narrowed from {:?} to {:?}",
                    row.get(),
                    span,
                    wider.unwrap()
                );
            }
        }
    }
}
