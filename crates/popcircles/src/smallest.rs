// Step 4 of application.md "Approach": the smallest circle containing a target share of a population.
// The search over radius is a climb from below and then a bisection over integer kilometres, driving
// `search`'s fixed-radius maximum at each probe. What is here is the question — a share, the population it
// resolves to, and the bracket every probe lives in; the I/O a resumed run needs is `smallest/cache.rs`'s.

use std::num::NonZeroU32;

use crate::geodesy::RadiusKm;
use crate::grid::{Col, Grid, Row};
use crate::kernel::Kernel;
use crate::progress::Progress;
use crate::search::{self, Candidate, SearchError};
use crate::table::Table;

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

/// The smallest circle found, and everything a caller needs to disagree with it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Smallest {
    /// The answer in whole kilometres, which is the unit the search steps in.
    pub radius_km: u32,
    pub radius: RadiusKm,
    /// The most populous centre at [`Self::radius_km`], by `search`'s tie-break.
    pub centre: Candidate,
    pub target: Target,
    /// `centre.population / target.total`, and zero for a table holding nobody — where every circle
    /// achieves the same nothing and a quotient would be a `NaN` standing for it.
    pub share_achieved: f64,
    /// The radius one kilometre short of the answer and the population it held, or `None` when the answer
    /// is 0 km and there is nothing below it.
    ///
    /// This is the **bracket the search proved**, and it is a stronger statement than the answer alone:
    /// the pair says this radius reaches the target and the one below it does not, both measured. That the
    /// answer is also the *minimum* rests on the computed predicate being monotone, which
    /// [`Self::predicate_slack_persons`] is the width of.
    pub short_below: Option<(u32, f64)>,
    /// Whether the answer's circle is the whole table, which every circle at [`CEILING_KM`] is and a
    /// narrower one on a small grid can be. A rendered circle that is the grid is worth knowing about.
    pub covers_whole_grid: bool,
    /// How far apart two radii's populations have to be for the comparison between them to be certain.
    ///
    /// See [`predicate_slack_persons`]. Reported rather than applied: inside it the computed predicate can
    /// invert, so the answer is the true minimum for a target further than this from a plateau in the
    /// radius, and a caller close to one wants the bracket rather than a comparison this search cannot
    /// make.
    pub predicate_slack_persons: f64,
    /// The population slack the fixed-radius search at the answer's radius pruned with — zero, #6's
    /// choice, carried up so a caller reads one result rather than this crate's constants.
    pub tolerance_persons: f64,
}

/// How far apart two circles' computed populations must be before the comparison between them is certain,
/// in persons.
///
/// **Not ADR 0003's 4 ulp**, which bounds one rectangle query. A circle is a sum of up to one query per
/// grid row, added in [`crate::circle::population`]'s fixed order, so the error composes: `rows · 4 ulp` of
/// the total for the terms, plus about `rows · ε · total` for the additions that combine them. At the
/// registry raster's 21 600 rows and a total near 8e9 that is about 0.12 persons — seven orders of
/// magnitude below the total, and five orders *above* the per-query figure, which is why quoting the
/// per-query figure for a circle would be wrong by a factor of 30 000.
///
/// Conservative in both terms: no cancellation is assumed, and `4 ulp(total)` is charged to every row
/// including the ones whose queries are over a fraction of it.
#[must_use]
pub fn predicate_slack_persons(grid: &Grid, total: f64) -> f64 {
    let rows = f64::from(grid.height());
    let ulp = total.next_up() - total;
    rows * 4.0 * ulp + rows * f64::EPSILON * total
}

/// The row `index` names.
///
/// A panic rather than a `Result` for [`search`]'s reason: a [`Grid`] has at least one row and one column,
/// so index 0 is a cell of every grid there is, and a miss would be a wiring mistake in this module rather
/// than an input a caller could act on.
fn north_west(grid: &Grid) -> (Row, Col) {
    match (grid.row(0), grid.col(0)) {
        (Some(row), Some(col)) => (row, col),
        _ => panic!("a grid has at least one row and one column"),
    }
}

/// `⌈log₂ width⌉`: how many halvings close an interval of `width` candidates.
fn halvings(width: u32) -> u32 {
    match width {
        0 | 1 => 0,
        _ => u32::BITS - (width - 1).leading_zeros(),
    }
}

/// An upper bound on the probes still to come once `km` is the next one the climb will take.
///
/// The climb doubles until it reaches the cap, and the bracket it hands the bisection is no wider than the
/// probe that closed it, so both terms are ceilings of a logarithm rather than counts.
fn climb_probes_remaining(km: u32) -> u32 {
    let cap = CEILING_KM - 1;
    let doublings = 1 + halvings(cap / km.max(1));
    doublings + halvings(km)
}

/// Whether a circle of `radius` centred on `centre`'s row spans every row of the grid in full.
///
/// Geometric rather than a comparison of populations: two different circles can hold the same people, and
/// what this field claims is coverage.
fn spans_whole_grid(grid: &Grid, centre: Row, radius: RadiusKm) -> Result<bool, SearchError> {
    let kernel = Kernel::new(*grid, centre, radius)?;
    let mut rows = 0u32;
    for (_, span) in kernel.rows() {
        if span != crate::kernel::Span::FullTurn {
            return Ok(false);
        }
        rows += 1;
    }
    Ok(rows == grid.height())
}

/// The maximum at `km`, from the ledger when it holds one and from a search when it does not.
fn probe<L: RadiusLedger>(
    table: &Table<'_>,
    km: u32,
    spacing: NonZeroU32,
    ledger: &mut L,
) -> Result<Candidate, SmallestError<L::Error>> {
    // Before the search and never after it, which is the whole of what a ledger buys.
    if let Some(found) = ledger.get(km) {
        return Ok(found);
    }
    // The inner search reports per level of its own refinement; two meters through one sink interleave
    // into noise, so this one is silent and the caller's sink hears about radii.
    let found = search::most_populous(table, RadiusKm::from(km), spacing, &mut ())?.centre;
    ledger.put(km, found).map_err(SmallestError::Ledger)?;
    Ok(found)
}

/// The smallest circle whose population reaches `share` of the table's own total, by whole kilometres of
/// ground radius.
///
/// The total is [`Table::whole`]'s query, so it is the figure a build publishes rather than a second
/// summation of the same cells, and the target is `share × total`.
///
/// **The probe order climbs from below**: 1, 2, 4, … kilometres until one reaches the target, then a
/// bisection of the bracket that closed. Not a bisection of `[0, CEILING_KM]`, and the reason is a
/// property of the layer below rather than a preference. `search`'s pruning is strict, so a plateau of
/// centres that all hold the same population is never pruned and refinement runs to single cells across
/// all of it; at radii covering most of the globe that plateau is most of the grid. Climbing keeps every
/// probe of an ordinary share inside the regime where the bound bites, and `CEILING_KM` — where every
/// centre ties and the plateau is the whole grid — is answered by one [`Table::whole`] query instead of
/// ever being searched.
///
/// `spacing` is forwarded to every probe untouched. There is no default here for the reason there is none
/// there: the useful range is a measured property of the raster and the radius.
///
/// `progress` is advanced once per settled radius, `(settled, settled + a bound on what remains)`. The
/// total is a bound rather than a count because how many probes there are depends on where the answer is,
/// and it may *rise* while the climb is still discovering how far it has to go — that is the honest shape
/// of a doubling search. It reaches the settled count exactly, so the last call a sink sees is `(n, n)`.
///
/// # Errors
/// [`SmallestError::Search`] when a probe cannot be searched — a grid whose columns do not close has no
/// kernels — and [`SmallestError::Ledger`] when the ledger cannot record one. There is no error for an
/// unreachable target: a [`Share`] is at most one and the ceiling holds the whole total.
pub fn smallest<L: RadiusLedger, P: Progress>(
    table: &Table<'_>,
    share: Share,
    spacing: NonZeroU32,
    ledger: &mut L,
    progress: &mut P,
) -> Result<Smallest, SmallestError<L::Error>> {
    let grid = *table.grid();
    let (whole_rows, whole_cols) = table.whole();
    let total = table.population(whole_rows, whole_cols);
    let target = Target::of(share, total);

    let mut settled = 0u64;
    // The climb. `CEILING_KM - 1` is the cap because the ceiling itself is not a radius this searches.
    let cap = CEILING_KM - 1;
    let mut km = 1u32;
    let mut low = 0u32;
    let mut short_below: Option<(u32, f64)> = None;
    let mut reached: Option<(u32, Candidate)> = None;

    loop {
        progress.advance(settled, settled + u64::from(climb_probes_remaining(km)));
        let found = probe(table, km, spacing, ledger)?;
        settled += 1;

        if found.population >= target.persons {
            reached = Some((km, found));
            break;
        }
        short_below = Some((km, found.population));
        low = km + 1;
        if km == cap {
            break;
        }
        km = km.saturating_mul(2).min(cap);
    }

    let Some((mut high, mut best)) = reached else {
        // Nothing under the ceiling reaches the target, so the answer is the circle that is the grid. Its
        // population is the extent's own query, and every cell of the grid is a maximiser of it — so
        // `Candidate::better`'s rule, ties to the smaller `(row, col)`, names the north-west one.
        let (row, col) = north_west(&grid);
        return Ok(Smallest {
            radius_km: CEILING_KM,
            radius: ceiling_radius(),
            centre: Candidate {
                row,
                col,
                population: total,
            },
            target,
            share_achieved: achieved(total, total),
            short_below,
            covers_whole_grid: true,
            predicate_slack_persons: predicate_slack_persons(&grid, total),
            tolerance_persons: 0.0,
        });
    };

    // The bisection. Every radius under `low` has been ruled out by a probe that fell short, and `high`
    // reaches, so the invariant holds at entry and the loop keeps it.
    while low < high {
        progress.advance(settled, settled + u64::from(halvings(high - low)));
        let mid = low + (high - low) / 2;
        let found = probe(table, mid, spacing, ledger)?;
        settled += 1;

        if found.population >= target.persons {
            high = mid;
            best = found;
        } else {
            short_below = Some((mid, found.population));
            low = mid + 1;
        }
    }

    progress.advance(settled, settled);
    Ok(Smallest {
        radius_km: high,
        radius: RadiusKm::from(high),
        centre: best,
        target,
        share_achieved: achieved(best.population, total),
        // Only the radius directly below the answer is a witness to its minimality; a short probe further
        // down is one the bisection has already superseded.
        short_below: short_below.filter(|(km, _)| Some(*km) == high.checked_sub(1)),
        covers_whole_grid: spans_whole_grid(&grid, best.row, RadiusKm::from(high))?,
        predicate_slack_persons: predicate_slack_persons(&grid, total),
        tolerance_persons: 0.0,
    })
}

/// The share a population is of a total, and zero when the total is nothing: every circle over an empty
/// table achieves the same nothing, and `0 / 0` would publish a `NaN` standing for it.
fn achieved(population: f64, total: f64) -> f64 {
    if total > 0.0 { population / total } else { 0.0 }
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

    fn spacing(cells: u32) -> NonZeroU32 {
        NonZeroU32::new(cells).expect("a fixture spacing is not zero")
    }

    fn share(value: f64) -> Share {
        Share::new(value).expect("a fixture share is a proportion")
    }

    /// A raster of zeros with `value` planted on each of `patches`, given as inclusive `(rows, cols)`.
    fn planted(
        patches: Vec<((u32, u32), (u32, u32))>,
        value: f32,
    ) -> impl Fn(u32, u32) -> f32 + use<> {
        move |row, col| {
            let inside = patches.iter().any(|((north, south), (first, last))| {
                (*north..=*south).contains(&row) && (*first..=*last).contains(&col)
            });
            if inside { value } else { 0.0 }
        }
    }

    /// The maximum at one radius, by the same search a probe runs, for a test that wants to check a
    /// bracket's other end without going through `smallest` again.
    fn maximum_at(table: &Table<'_>, km: u32, cells: u32) -> Candidate {
        search::most_populous(table, RadiusKm::from(km), spacing(cells), &mut ())
            .expect("a whole-globe fixture has kernels")
            .centre
    }

    fn found(table: &Table<'_>, value: f64, cells: u32) -> Smallest {
        smallest(table, share(value), spacing(cells), &mut (), &mut ())
            .expect("a whole-globe fixture and a no-op ledger cannot fail")
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

    #[test]
    fn the_answer_reaches_the_target_and_the_kilometre_below_it_does_not() {
        // The bracket, on two fixtures, with every figure a literal: the answer, the population it holds
        // and the population the kilometre below it holds. Changing any of the three fails, which is what
        // makes this a claim about the search rather than a record of what it printed.
        //
        // The cluster is #6's: four cells of a hundred at 5 N to 5 S, whose diagonal is 14.13 degrees, or
        // 1571 km. So no centre reaches all four at 1571 — three of them is 300 people — and 1572 is the
        // first radius that does.
        let grid = grid();
        let cluster = payload_over(&grid, planted(vec![((8, 9), (15, 16))], 100.0));
        let table = Table::new(grid, &cluster).expect("the build emits the padded product");

        let all = found(&table, 1.0, 4);
        assert_eq!(all.radius_km, 1572);
        assert_eq!(all.centre.population, 400.0);
        assert_eq!(all.short_below, Some((1571, 300.0)));
        assert_eq!(all.share_achieved, 1.0);
        assert!(!all.covers_whole_grid);
        assert_eq!(all.tolerance_persons, 0.0);
        // The witness is a probe and not an inference: the same radius searched directly is short too.
        assert_eq!(maximum_at(&table, 1571, 4).population, 300.0);

        // And half of a whole-globe fixture, where the answer is a long way from either end of the
        // bracket: 210 276 people over 648 distinct cells, half of them inside 5770 km of one centre.
        let payload = payload_over(&grid, distinct(&grid));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");
        let half = found(&table, 0.5, 4);
        assert_eq!(half.radius_km, 5770);
        assert_eq!(half.target.persons, 105_138.0);
        assert_eq!(half.centre.population, 105_623.0);
        assert_eq!(half.short_below, Some((5769, 104_706.0)));
        assert_eq!(maximum_at(&table, 5769, 4).population, 104_706.0);
    }

    #[test]
    fn no_radius_under_the_answer_reaches_the_target() {
        // The minimality claim by exhaustion rather than by the bracket: every one of the 1572 integer
        // radii below the answer is searched and every one of them falls short. This is the test the
        // climb-then-bisect order has to survive — a bracket closed one kilometre too high would pass the
        // test above and fail here.
        //
        // Measured on this tree at 0.98 s in a debug build, which is why the fixture is the cluster and
        // not the half-share above: the same scan at 5770 km would be nearly four times that.
        let grid = grid();
        let payload = payload_over(&grid, planted(vec![((8, 9), (15, 16))], 100.0));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let answer = found(&table, 1.0, 4);
        assert_eq!(answer.radius_km, 1572);
        for km in 0..answer.radius_km {
            let held = maximum_at(&table, km, 4).population;
            assert!(
                held < answer.target.persons,
                "{km} km already holds {held} of the {} wanted",
                answer.target.persons
            );
        }
    }

    #[test]
    fn a_share_one_cell_holds_is_answered_at_zero_kilometres() {
        // Zero is inside the bracket rather than a case beside it: the climb's first reaching radius is
        // 1 km, the bisection over [0, 1] probes zero, and a circle that is its own centre cell is the
        // answer. `short_below` is `None` because there is no radius below it to have proved short.
        let grid = grid();
        let payload = payload_over(&grid, planted(vec![((4, 4), (7, 7))], 50.0));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let single = found(&table, 1.0, 4);
        assert_eq!(single.radius_km, 0);
        assert_eq!(single.radius.km(), 0.0);
        assert_eq!(single.short_below, None);
        assert_eq!((single.centre.row.get(), single.centre.col.get()), (4, 7));
        assert_eq!(single.centre.population, 50.0);
    }

    #[test]
    fn a_target_only_the_whole_grid_reaches_is_answered_at_the_ceiling() {
        // The fixture's own geometry is what makes this case reachable, and it is worth stating: the grid
        // is symmetric about the equator and the antimeridian, so every cell centre has another cell
        // centre exactly antipodal to it — (85 N, 175 W) against (85 S, 5 E). Every cell holds people, so
        // a circle containing all of them spans half the circumference, and no integer kilometre under
        // 20 016 does. At 20 015 the maximum misses exactly one cell, and on this fixture that cell is the
        // one holding a single person.
        let grid = grid();
        let payload = payload_over(&grid, distinct(&grid));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");
        let (rows, cols) = table.whole();
        let total = table.population(rows, cols);

        let whole = found(&table, 1.0, 4);
        assert_eq!(whole.radius_km, CEILING_KM);
        assert!(whole.covers_whole_grid);
        // The population is the extent's own query, bit for bit, and not a fold that agrees to a
        // tolerance — which is the whole reason the ceiling is answered the way it is.
        assert_eq!(whole.centre.population.to_bits(), total.to_bits());
        assert_eq!(whole.target.persons.to_bits(), total.to_bits());
        assert_eq!(whole.share_achieved, 1.0);
        // The north-west cell, by the tie-break the test below pins.
        assert_eq!((whole.centre.row.get(), whole.centre.col.get()), (0, 0));
        assert_eq!(whole.short_below, Some((20_015, total - 1.0)));
    }

    #[test]
    fn the_tie_break_over_equal_candidates_names_the_north_west_cell() {
        // What licenses the centre above without re-deriving `search`'s rule here: with every candidate
        // holding the same population, folding the real rule over every cell of the grid gives cell
        // (0, 0). Folded in both directions, because the rule is order-independent and a shortcut resting
        // on a first-seen rule would be wrong the moment the traversal changed.
        let grid = grid();
        let every: Vec<Candidate> = grid
            .rows()
            .flat_map(|row| {
                grid.cols().map(move |col| Candidate {
                    row,
                    col,
                    population: 7.0,
                })
            })
            .collect();

        let forwards = every
            .iter()
            .copied()
            .reduce(Candidate::better)
            .expect("the grid has cells");
        let backwards = every
            .iter()
            .rev()
            .copied()
            .reduce(Candidate::better)
            .expect("the grid has cells");

        assert_eq!((forwards.row.get(), forwards.col.get()), (0, 0));
        assert_eq!((backwards.row.get(), backwards.col.get()), (0, 0));
    }

    #[test]
    fn the_same_search_twice_gives_the_same_bits() {
        let grid = grid();
        let payload = payload_over(&grid, distinct(&grid));
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        let first = found(&table, 0.25, 5);
        let second = found(&table, 0.25, 5);
        assert_eq!(first.radius_km, second.radius_km);
        assert_eq!(
            first.centre.population.to_bits(),
            second.centre.population.to_bits()
        );
        assert_eq!(first.centre.row, second.centre.row);
        assert_eq!(first.centre.col, second.centre.col);
        assert_eq!(first.short_below, second.short_below);
    }
}
