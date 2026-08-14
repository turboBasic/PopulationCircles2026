// Step 4 of application.md "Approach": the smallest circle containing a target share of a population.
// The search over radius is a climb from below and then a bisection over integer kilometres, driving
// `search`'s fixed-radius maximum at each probe. What is here is the question — a share, the population it
// resolves to, and the bracket every probe lives in; the I/O a resumed run needs is `smallest/cache.rs`'s.

use crate::geodesy::RadiusKm;
use crate::search::{Candidate, SearchError};

#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum ShareError {
    #[error("a population share must be finite; {share} is not")]
    NotFinite { share: f64 },

    #[error("a population share must be above zero; {share} is not")]
    NotPositive { share: f64 },

    #[error("a population share must not exceed one; {share} does")]
    AboveOne { share: f64 },
}

/// The share of a population a circle is asked to contain, checked once where it is made.
///
/// Zero is refused rather than answered. A circle holding nobody is satisfied by every radius there is,
/// zero included, so the answer would be a property of this type rather than of the raster. One is
/// accepted, and [`CEILING_KM`] is what makes it answerable.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Share(f64);

impl Share {
    /// # Errors
    /// [`ShareError`] when the value is not a proportion: not finite, at or below zero, or above one.
    pub fn new(share: f64) -> Result<Self, ShareError> {
        if !share.is_finite() {
            return Err(ShareError::NotFinite { share });
        }
        if share <= 0.0 {
            return Err(ShareError::NotPositive { share });
        }
        if share > 1.0 {
            return Err(ShareError::AboveOne { share });
        }
        Ok(Self(share))
    }

    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// A share resolved against the population it is a share of.
///
/// The total is the table's own whole extent, which is the figure a build publishes, so there is one
/// answer in this program to how many people there are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    pub share: Share,
    /// `share × total`, which at a share of one is `total` exactly: multiplying by 1.0 is the identity in
    /// IEEE arithmetic, and that exactness is what makes a whole-population target reachable rather than
    /// a rounding away from it.
    pub persons: f64,
    pub total: f64,
}

impl Target {
    #[must_use]
    pub fn of(share: Share, total: f64) -> Self {
        Self {
            share,
            persons: share.get() * total,
            total,
        }
    }
}

/// The radius at which a circle is the whole grid, in kilometres, and the top of every bracket.
///
/// Half the circumference is `EARTH_RADIUS_KM · π` = 20 015.11 km, and [`crate::kernel::Kernel`] clamps a
/// cap's angle at half a turn, so at or past that figure every row of the grid is spanned in full from any
/// centre whatsoever. 20 016 is the first integer kilometre past it.
///
/// A circle this wide is never *searched*: every centre holds the same population, so nothing prunes and a
/// search would refine the grid to single cells. Its population is [`crate::table::Table::whole`]'s single
/// query instead — the circle is the whole extent, so that query is its population.
pub const CEILING_KM: u32 = 20_016;

/// [`CEILING_KM`] as a radius.
#[must_use]
pub fn ceiling_radius() -> RadiusKm {
    RadiusKm::from(CEILING_KM)
}

/// Where the maximum found at a radius is kept, so an interrupted run resumes instead of paying for it
/// twice.
///
/// A trait rather than a type for [`crate::raster::RasterSource`]'s reason: what the search needs of a
/// ledger is these two operations, and a fixture, an in-memory map and the JSON document
/// [`cache`](crate::smallest::cache) publishes are then the same seam. The error is associated because a
/// ledger's failures are its own — this module has no vocabulary for a filesystem — which is the shape
/// [`crate::table::build`] takes with its sink.
///
/// **What may be stored under a radius is the maximum over the table's cell centres at that radius**, and
/// nothing else. It is a property of the table alone, so it is independent of the share that asked for it
/// and of the initial spacing the search ran with — `search`'s own tests pin the second — which is why
/// neither appears in a key here or in the identity a stored ledger is checked against.
pub trait RadiusLedger {
    /// What a ledger fails with. [`std::convert::Infallible`] for one that cannot.
    type Error;

    /// The maximum recorded at `km`, or `None` when this radius has not been evaluated.
    fn get(&self, km: u32) -> Option<Candidate>;

    /// Records the maximum at `km`, durably enough that a run interrupted after this returns finds it.
    ///
    /// # Errors
    /// Whatever the implementation cannot do; a run stops rather than continuing with a ledger that has
    /// silently stopped recording.
    fn put(&mut self, km: u32, found: Candidate) -> Result<(), Self::Error>;
}

/// The ledger for a caller that wants no resumption, so the search takes `()` instead of growing a second
/// signature without the parameter — [`crate::progress::Progress`]'s reason, and the same shape.
impl RadiusLedger for () {
    type Error = std::convert::Infallible;

    fn get(&self, _km: u32) -> Option<Candidate> {
        None
    }

    fn put(&mut self, _km: u32, _found: Candidate) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Why a search for the smallest circle stopped.
///
/// Two arms, and there is no third for a target nothing reaches: the ceiling is the whole grid and a
/// [`Share`] is at most one, so every target this can be asked for is met by some radius in the bracket.
/// The ledger's own failure keeps its own type, so nothing here needs a vocabulary for whichever ledger a
/// caller brought — [`crate::table::BuildError`]'s shape.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum SmallestError<E> {
    #[error("a probe of the bracket could not be searched")]
    Search(#[from] SearchError),

    #[error("the radius ledger could not record a probe")]
    Ledger(#[source] E),
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this narrow
// exemption; docs/ai/code.md allows both in tests. float_cmp is the point rather than a concession: the
// fixtures below are distinct small integers, so every partial sum is exact and a tolerance would let a
// fold that lost a row pass.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_precision_loss
)]
mod tests {
    use std::collections::BTreeMap;
    use std::convert::Infallible;

    use super::*;
    use crate::circle;
    use crate::geodesy::LatLon;
    use crate::grid::{Col, Grid, Row};
    use crate::kernel::{Kernel, Span};
    use crate::raster::Synthetic;
    use crate::table::{Decimation, Table, build};

    /// A ledger in a map, which is what the tests here drive a search with: the seam without the
    /// filesystem, so what a resumed run does is pinned before `cache.rs` exists.
    #[derive(Debug, Default)]
    struct Recorded {
        entries: BTreeMap<u32, Candidate>,
    }

    impl RadiusLedger for Recorded {
        type Error = Infallible;

        fn get(&self, km: u32) -> Option<Candidate> {
            self.entries.get(&km).copied()
        }

        fn put(&mut self, km: u32, found: Candidate) -> Result<(), Self::Error> {
            self.entries.insert(km, found);
            Ok(())
        }
    }

    /// The whole-globe fixture the search's own tests use, ten degrees a side.
    fn grid() -> Grid {
        Grid::new(
            36,
            18,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            10.0,
            -10.0,
        )
        .expect("a 36 x 18 whole-globe grid is valid")
    }

    /// `search.rs`'s band shape: closing in longitude, thirty degrees of latitude, so a cap is clipped at
    /// the grid's edge rather than refused. The ceiling has to close this grid too, because a table over a
    /// band is one `Kernel` accepts.
    fn band_grid() -> Grid {
        Grid::new(
            360,
            30,
            LatLon {
                lat: 60.0,
                lon: -180.0,
            },
            1.0,
            -1.0,
        )
        .expect("a band grid that closes in longitude is valid")
    }

    /// The registry raster's sentinel, so a fixture reaches the table by the path a real raster takes.
    const NODATA: f32 = -3.402_823e38;

    /// Distinct at every position, so every partial sum is exact in f64 and a cell counted twice moves a
    /// total.
    fn distinct(grid: &Grid) -> impl Fn(u32, u32) -> f32 + use<> {
        let width = grid.width();
        move |row, col| (row * width + col + 1) as f32
    }

    fn payload_over(grid: &Grid, cell: impl Fn(u32, u32) -> f32) -> Vec<f64> {
        let rows: Vec<Vec<f32>> = (0..grid.height())
            .map(|row| (0..grid.width()).map(|col| cell(row, col)).collect())
            .collect();
        let source = Synthetic::new(*grid, NODATA, rows).expect("the rows are the grid's shape");
        let mut payload = Vec::new();
        build(source, Decimation::none(*grid), &mut (), |row| {
            payload.extend_from_slice(row);
            Ok::<(), Infallible>(())
        })
        .expect("neither a synthetic source nor this sink can fail");
        payload
    }

    fn col_of(grid: &Grid, index: u32) -> Col {
        grid.col(index).expect("a column of the fixture")
    }

    fn row_of(grid: &Grid, index: u32) -> Row {
        grid.row(index).expect("a row of the fixture")
    }

    fn candidate(grid: &Grid, row: u32, col: u32, population: f64) -> Candidate {
        Candidate {
            row: row_of(grid, row),
            col: col_of(grid, col),
            population,
        }
    }

    #[test]
    fn a_share_is_a_proportion_and_nothing_else() {
        assert_eq!(Share::new(0.5).unwrap().get(), 0.5);
        // One is a share, and the reason this test states it rather than leaving it to the range: it is
        // the case the whole design of the ceiling exists to answer.
        assert_eq!(Share::new(1.0).unwrap().get(), 1.0);

        assert_eq!(
            Share::new(0.0).unwrap_err(),
            ShareError::NotPositive { share: 0.0 }
        );
        assert_eq!(
            Share::new(-0.25).unwrap_err(),
            ShareError::NotPositive { share: -0.25 }
        );
        assert!(matches!(
            Share::new(1.000_000_1),
            Err(ShareError::AboveOne { .. })
        ));
        assert!(matches!(
            Share::new(f64::NAN),
            Err(ShareError::NotFinite { .. })
        ));
        assert!(matches!(
            Share::new(f64::INFINITY),
            Err(ShareError::NotFinite { .. })
        ));
    }

    #[test]
    fn a_whole_share_targets_the_total_exactly() {
        // The identity that keeps a whole-population target reachable: no rounding stands between the
        // share and the figure the ceiling answers with.
        let total = 7_757_982_599.32;
        let whole = Target::of(Share::new(1.0).unwrap(), total);
        assert_eq!(whole.persons, total);
        assert_eq!(whole.persons.to_bits(), total.to_bits());

        let half = Target::of(Share::new(0.5).unwrap(), total);
        assert_eq!(half.persons, total / 2.0);
        assert_eq!(half.total, total);
    }

    #[test]
    fn a_ledger_gives_back_the_maximum_it_was_given_and_nothing_for_a_radius_it_has_not_seen() {
        let grid = grid();
        let mut ledger = Recorded::default();
        let found = candidate(&grid, 3, 7, 4200.0);

        assert_eq!(ledger.get(1500), None);
        ledger.put(1500, found).unwrap();
        assert_eq!(ledger.get(1500), Some(found));
        // The neighbouring kilometres are not the same probe, which is the whole of what an integer key
        // means here.
        assert_eq!(ledger.get(1499), None);
        assert_eq!(ledger.get(1501), None);
    }

    #[test]
    fn the_no_op_ledger_records_nothing() {
        // There is no behaviour to assert beyond this, and it is the reason the implementation exists: a
        // caller wanting no resumption passes `()` and the search needs no second signature. The `put`
        // succeeding while the `get` stays empty is what makes it a sink rather than a store.
        let grid = grid();
        let mut none = ();
        none.put(1500, candidate(&grid, 0, 0, 1.0)).unwrap();
        assert_eq!(RadiusLedger::get(&none, 1500), None);
    }

    #[test]
    fn the_ceiling_spans_every_row_of_a_grid_in_full() {
        // The claim `CEILING_KM` rests on, from every centre row rather than from one: a cap clamped at
        // half a turn reaches every row of the grid and closes each of them, so the circle is the grid.
        for grid in [grid(), band_grid()] {
            for centre in grid.rows() {
                let kernel =
                    Kernel::new(grid, centre, ceiling_radius()).expect("a grid that closes");
                let spans: Vec<(_, Span)> = kernel.rows().collect();

                assert_eq!(
                    spans.len(),
                    grid.height() as usize,
                    "the ceiling misses a row of a {}-row grid",
                    grid.height()
                );
                assert!(
                    spans.iter().all(|(_, span)| *span == Span::FullTurn),
                    "the ceiling leaves a row short of a full turn"
                );
            }
        }
    }

    #[test]
    fn the_ceiling_circle_is_the_tables_whole_extent() {
        // What licenses answering the ceiling with one `Table::whole` query instead of a fold over the
        // rows: the two are the same population. Here they agree bit for bit, because the fixture's sums
        // are exact; at full magnitude they agree to the bound `full_resolution_table.rs` measures, which
        // is the whole reason the query is preferred to the fold.
        for grid in [grid(), band_grid()] {
            let payload = payload_over(&grid, distinct(&grid));
            let table = Table::new(grid, &payload).expect("the build emits the padded product");
            let (rows, cols) = table.whole();
            let whole = table.population(rows, cols);

            // Three centres, one of them the first column, because a circle wide enough to close every
            // row must not depend on where it is centred — and the seam is where a fold that assembled a
            // closed row from two pieces would double count.
            for centre in [0, 17, 35] {
                let kernel = Kernel::new(grid, grid.middle_row(), ceiling_radius())
                    .expect("a grid that closes");
                let folded = circle::population(&table, &kernel, col_of(&grid, centre));
                assert_eq!(
                    folded.to_bits(),
                    whole.to_bits(),
                    "centre {centre} folds to {folded}, not the extent's {whole}"
                );
            }
        }
    }
}
