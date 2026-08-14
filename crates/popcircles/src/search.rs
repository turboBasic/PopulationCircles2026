// Step 3 of application.md "Approach": the most populous circle of a fixed ground radius, as a branch
// and bound over a refining grid of candidate centres.
//
// The multiresolution grid the technique needs is the grid of *centres*, not a second summation table: a
// rectangle of candidate cells carries one bound for every centre in it, and a rectangle that survives
// halves. So what is here is the rectangle, the slack it spans, and the level loop over the two — the
// circles themselves are `circle::population`'s and the geometry is `kernel`'s.

use std::num::NonZeroU32;

use crate::geodesy::arc_km;
use crate::grid::{Col, Grid, Row};
use crate::table::RowBand;

/// The relative amount [`slack_km`] inflates its answer by, so that the figure it returns dominates the
/// mathematical bound rather than approximating it.
///
/// 32 ε is 7.1e-15, which at a 20 000 km bound is 0.14 µm of ground. That is enormous next to the handful
/// of rounding steps in the expression it corrects — two `to_radians`, a `cos`, a multiply and an add — and
/// negligible next to a 30 arc-second cell's ~900 m, so the only cell the inflation can newly admit is one
/// whose distance sits within 0.14 µm of the boundary. Which is the cell it exists to admit.
const SLACK_MARGIN: f64 = 32.0 * f64::EPSILON;

/// The row `index` names.
///
/// A panic rather than a `Result` for [`crate::table::Table::population`]'s reason: every index reaching
/// this lies between two rows the grid itself minted, so a miss is a wiring mistake in this module and
/// not an input a caller could do anything with.
fn row_of(grid: &Grid, index: u32) -> Row {
    match grid.row(index) {
        Some(row) => row,
        None => panic!("row {index} is not a row of a {}-row grid", grid.height()),
    }
}

/// The column `index` names, a stop for [`row_of`]'s reason.
fn col_of(grid: &Grid, index: u32) -> Col {
    match grid.col(index) {
        Some(col) => col,
        None => panic!(
            "column {index} is not a column of a {}-column grid",
            grid.width()
        ),
    }
}

/// A rectangle of candidate centres: the unit a bound speaks for, and the unit that halves.
///
/// The columns are `first` and `last` as indices rather than a [`crate::table::ColSpan`]'s compass pair,
/// and **a block never wraps the seam**: blocks tile the index space from column 0, so the last block of
/// a row is short rather than wrapped. The circles a block's centres carry do wrap, which is exactly why
/// the rectangle of centres must not — a wrapped block would have two spellings of "the cells I speak
/// for" and the bound would have to pick one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    rows: RowBand,
    first: Col,
    last: Col,
}

impl Block {
    /// Any pair of columns is a block, so the constructor orders them rather than refusing one order —
    /// [`RowBand::new`]'s reason, and it leaves nothing here for a caller to get wrong.
    #[must_use]
    pub fn new(rows: RowBand, a: Col, b: Col) -> Self {
        Self {
            rows,
            first: a.min(b),
            last: a.max(b),
        }
    }

    #[must_use]
    pub const fn rows(self) -> RowBand {
        self.rows
    }

    #[must_use]
    pub const fn first(self) -> Col {
        self.first
    }

    #[must_use]
    pub const fn last(self) -> Col {
        self.last
    }

    #[must_use]
    pub const fn is_cell(self) -> bool {
        self.rows.north().get() == self.rows.south().get() && self.first.get() == self.last.get()
    }

    /// The grid tiled into blocks `spacing` cells on a side, row-major, the last of each extent short
    /// rather than overhanging.
    ///
    /// # Panics
    /// If `grid` is not the grid the returned blocks are read against — it is, by construction here, and
    /// the panic is [`row_of`]'s.
    #[must_use]
    pub fn tile(grid: &Grid, spacing: NonZeroU32) -> Vec<Self> {
        // Saturating, because a spacing larger than the grid is a legal way to ask for one block and
        // `north + step` would otherwise wrap on a spacing near u32::MAX.
        let step = spacing.get();
        let mut blocks = Vec::new();
        let mut north = 0u32;
        while north < grid.height() {
            let south = north.saturating_add(step - 1).min(grid.height() - 1);
            let mut first = 0u32;
            while first < grid.width() {
                let last = first.saturating_add(step - 1).min(grid.width() - 1);
                blocks.push(Self::new(
                    RowBand::new(row_of(grid, north), row_of(grid, south)),
                    col_of(grid, first),
                    col_of(grid, last),
                ));
                first = last + 1;
            }
            north = south + 1;
        }
        blocks
    }

    /// The cell whose circle stands for the block's: the floor midpoint of each extent.
    ///
    /// Floor rather than round, so an extent of even length takes the north-western of its two middles.
    /// Which one is arbitrary; that it is the same one every run is not.
    ///
    /// # Panics
    /// If `grid` is not the grid this block's indices were minted by; [`row_of`] says why that is a stop.
    #[must_use]
    pub fn probe(self, grid: &Grid) -> (Row, Col) {
        let (north, south) = (self.rows.north().get(), self.rows.south().get());
        let (first, last) = (self.first.get(), self.last.get());
        (
            row_of(grid, north + (south - north) / 2),
            col_of(grid, first + (last - first) / 2),
        )
    }

    /// The block halved in each extent longer than one cell — four children, or two when one extent is
    /// already a single row or column, or none at all for a single cell.
    ///
    /// Nothing for a single cell is what terminates the refinement: every split shortens the longer
    /// extent, so a block reaches one cell in a bounded number of rounds and then stops producing work.
    ///
    /// # Panics
    /// If `grid` is not the grid this block's indices were minted by; [`row_of`] says why that is a stop.
    pub fn split(self, grid: &Grid) -> impl Iterator<Item = Self> {
        let halves = |low: u32, high: u32| {
            if low == high {
                vec![(low, high)]
            } else {
                let mid = low + (high - low) / 2;
                vec![(low, mid), (mid + 1, high)]
            }
        };

        let children = if self.is_cell() {
            Vec::new()
        } else {
            let rows = halves(self.rows.north().get(), self.rows.south().get());
            let cols = halves(self.first.get(), self.last.get());
            rows.into_iter()
                .flat_map(|(north, south)| {
                    cols.iter().map(move |&(first, last)| {
                        Self::new(
                            RowBand::new(row_of(grid, north), row_of(grid, south)),
                            col_of(grid, first),
                            col_of(grid, last),
                        )
                    })
                })
                .collect()
        };
        children.into_iter()
    }
}

/// A candidate centre and the population of its circle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    pub row: Row,
    pub col: Col,
    pub population: f64,
}

impl Candidate {
    /// The better of two candidates: more population wins, and equal population keeps the smaller
    /// `(row, col)`.
    ///
    /// A comparison rather than "keep the first one seen", and the difference is the answer's
    /// independence from the order candidates arrive in. Pruning changes that order with every initial
    /// spacing, so a rule reading it would give a different centre for the same raster and radius. This
    /// one is symmetric — `a.better(b)` and `b.better(a)` are the same candidate — which is what lets the
    /// search fold in whatever order the traversal happens to produce.
    #[must_use]
    pub fn better(self, other: Self) -> Self {
        // A population that is not a number takes neither comparison and falls through to the position,
        // so a table holding one yields a deterministic answer rather than an order-dependent one. It
        // cannot arise from a sanitised raster; what matters here is that it has no way to be undefined.
        if other.population > self.population {
            other
        } else if other.population < self.population {
            self
        } else if (other.row, other.col) < (self.row, self.col) {
            other
        } else {
            self
        }
    }
}

/// An upper bound on the ground distance from `block`'s probe to any cell centre in it.
///
/// Two hops and the triangle inequality on the sphere. Take the probe at `(φ₀, λ₀)` and a target cell
/// centre at `(φ, λ)`: go along the probe's own parallel to `(φ₀, λ)`, then along that meridian to
/// `(φ, λ)`. The first leg is a path of length `R · Δλ · cos φ₀` — the parallel is not a great circle away
/// from the equator, so the geodesic between its ends is no longer than the arc along it — and the second
/// leg is exactly `R · Δφ`. A geodesic between the ends is no longer than any path joining them, so the
/// sum bounds it.
///
/// The cosine is the **probe's** because the longitude leg stays at the probe's latitude. That is what
/// makes the bound tighten toward the poles rather than needing the block's worst latitude, and it is why
/// a block of the same index extent bounds a much shorter distance at 80° than at the equator.
///
/// Offsets are between **cell centres**, not out to the block's outer boundary: the candidates a bound
/// speaks for are cell centres, so measuring to a cell edge would loosen it for nothing.
///
/// The answer is inflated by [`SLACK_MARGIN`], and the reason is that the inequality above is not strict
/// in two configurations. A block one column wide has `Δλ = 0`, so the two-hop path *is* the meridian
/// geodesic; a block one row tall on the equator has `Δφ = 0` and the parallel *is* a great circle. In
/// both the mathematical margin is zero, so a figure computed one ulp light would exclude a cell that is
/// genuinely within reach — and a block holding the maximum would be pruned.
///
/// Where the bound stops discriminating: once `radius + slack` reaches half the circumference the widened
/// circle is the whole sphere, every bound equals the raster's total and no block is ever pruned. That is
/// the ceiling on a useful initial spacing rather than a correctness limit.
///
/// # Panics
/// If `grid` is not the grid `block`'s indices were minted by; [`row_of`] says why that is a stop.
#[must_use]
pub fn slack_km(grid: &Grid, block: Block) -> f64 {
    let (probe_row, probe_col) = block.probe(grid);
    let probe_lat = grid.centre_lat(probe_row);

    let north = grid.centre_lat(block.rows().north());
    let south = grid.centre_lat(block.rows().south());
    let delta_lat_deg = (north - probe_lat).abs().max((south - probe_lat).abs());

    // The probe is a cell of its own block, so neither difference underflows.
    let west = probe_col.get() - block.first().get();
    let east = block.last().get() - probe_col.get();
    let delta_lon_deg = f64::from(west.max(east)) * grid.lon_step().abs();

    let two_hop_rad =
        delta_lat_deg.to_radians() + delta_lon_deg.to_radians() * probe_lat.to_radians().cos();
    arc_km(two_hop_rad) * (1.0 + SLACK_MARGIN)
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this narrow
// exemption; docs/ai/code.md allows both in tests. float_cmp is for the exactly-zero and tightness
// assertions, where the value being exactly what it is is the property.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_precision_loss
)]
mod tests {
    use std::collections::BTreeSet;
    use std::convert::Infallible;

    use proptest::prelude::*;

    use super::*;
    use crate::circle;
    use crate::geodesy::{LatLon, RadiusKm, great_circle_km};
    use crate::kernel::Kernel;
    use crate::raster::Synthetic;
    use crate::table::{Decimation, Table, build};

    const WIDTH: u32 = 36;
    const HEIGHT: u32 = 18;

    /// Ten degrees a side, the same whole-globe shape `circle.rs` measures against, and small enough
    /// that a tiling can be compared against every cell of the grid.
    fn grid() -> Grid {
        Grid::new(
            WIDTH,
            HEIGHT,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            10.0,
            -10.0,
        )
        .expect("a 36 x 18 whole-globe grid is valid")
    }

    fn spacing(cells: u32) -> NonZeroU32 {
        NonZeroU32::new(cells).expect("a fixture spacing is not zero")
    }

    /// Every cell a block speaks for, expanded.
    fn cells_of(block: Block) -> Vec<(u32, u32)> {
        (block.rows().north().get()..=block.rows().south().get())
            .flat_map(|row| (block.first().get()..=block.last().get()).map(move |col| (row, col)))
            .collect()
    }

    fn every_cell() -> BTreeSet<(u32, u32)> {
        (0..HEIGHT)
            .flat_map(|row| (0..WIDTH).map(move |col| (row, col)))
            .collect()
    }

    #[test]
    fn a_tiling_covers_every_cell_exactly_once() {
        // Both halves matter and neither implies the other: the set says nothing was missed, and the
        // count says nothing was covered twice. Spacings 5 and 18 are the ones that do not divide the
        // grid evenly in one direction or the other, which is where a short final block is either
        // right or an off-by-one.
        let grid = grid();
        for cells in [1u32, 4, 5, 18, 40] {
            let blocks = Block::tile(&grid, spacing(cells));
            let covered: Vec<(u32, u32)> = blocks.iter().copied().flat_map(cells_of).collect();

            assert_eq!(
                covered.iter().copied().collect::<BTreeSet<(u32, u32)>>(),
                every_cell(),
                "spacing {cells} misses or invents a cell"
            );
            assert_eq!(
                covered.len(),
                (WIDTH * HEIGHT) as usize,
                "spacing {cells} covers a cell twice"
            );
        }
    }

    #[test]
    fn a_spacing_past_the_grid_is_one_block() {
        // The saturating arm, and the shape a caller asking for "the whole globe as one candidate" gets.
        let grid = grid();
        let blocks = Block::tile(&grid, spacing(u32::MAX));
        assert_eq!(blocks.len(), 1);
        assert_eq!(cells_of(blocks[0]).len(), (WIDTH * HEIGHT) as usize);
    }

    #[test]
    fn a_single_cell_block_splits_into_nothing() {
        // The termination condition, asserted directly rather than inferred from the round count below.
        let grid = grid();
        for block in Block::tile(&grid, spacing(1)) {
            assert!(block.is_cell());
            assert_eq!(block.split(&grid).count(), 0);
        }
    }

    #[test]
    fn splitting_reaches_single_cells_in_five_rounds_and_loses_no_cell() {
        // An extent of 18 halves 18, 9, 5, 3, 2, 1 — five rounds — and the whole-globe fixture at
        // spacing 18 is two such blocks. The round count is stated because an off-by-one in `halves`
        // that still terminates would take six, and because a `mid` computed the other way round would
        // leave a child of the parent's own length and never terminate at all.
        let grid = grid();
        let mut level = Block::tile(&grid, spacing(18));
        let mut rounds = 0;

        while !level.iter().all(|block| block.is_cell()) {
            level = level
                .iter()
                .flat_map(|block| {
                    if block.is_cell() {
                        vec![*block]
                    } else {
                        block.split(&grid).collect()
                    }
                })
                .collect();
            rounds += 1;
            assert!(rounds <= 8, "splitting is not converging: {rounds} rounds");
        }

        assert_eq!(rounds, 5);
        // Splitting is a partition at every round, not merely a shrinking: the leaves are the grid.
        let leaves: Vec<(u32, u32)> = level.iter().copied().flat_map(cells_of).collect();
        assert_eq!(
            leaves.iter().copied().collect::<BTreeSet<(u32, u32)>>(),
            every_cell()
        );
        assert_eq!(leaves.len(), (WIDTH * HEIGHT) as usize);
    }

    #[test]
    fn a_probe_on_an_even_extent_is_the_north_western_middle() {
        // Four rows and four columns, so both extents have two middles and the choice is visible. Rows
        // 1 and 2 are the middles of 0..3, and 1 is the northern.
        let grid = grid();
        let block = Block::new(
            RowBand::new(row_of(&grid, 0), row_of(&grid, 3)),
            col_of(&grid, 0),
            col_of(&grid, 3),
        );
        let (row, col) = block.probe(&grid);
        assert_eq!((row.get(), col.get()), (1, 1));

        // And an odd extent has one middle, which is the case the floor must not shift.
        let odd = Block::new(
            RowBand::new(row_of(&grid, 4), row_of(&grid, 8)),
            col_of(&grid, 10),
            col_of(&grid, 14),
        );
        let (row, col) = odd.probe(&grid);
        assert_eq!((row.get(), col.get()), (6, 12));
    }

    #[test]
    fn a_probe_is_a_cell_of_the_block_it_stands_for() {
        // The property every bound rests on and the one a midpoint formula can lose at a short final
        // block: the probe is inside its own rectangle.
        let grid = grid();
        for cells in [1u32, 4, 5, 18] {
            for block in Block::tile(&grid, spacing(cells)) {
                let (row, col) = block.probe(&grid);
                assert!(
                    cells_of(block).contains(&(row.get(), col.get())),
                    "spacing {cells}: probe {:?} is outside its block",
                    (row.get(), col.get())
                );
            }
        }
    }

    /// A whole globe with a row centred on the equator, which needs an odd row count: centres sit at
    /// `90 − (r + 0.5)·20`, so row 4 is at 0.0 exactly. That row is one of the two configurations where
    /// the two-hop bound is not strict.
    fn equatorial_grid() -> Grid {
        Grid::new(
            WIDTH,
            9,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            10.0,
            -20.0,
        )
        .expect("a 36 x 9 whole-globe grid is valid")
    }

    fn block_of(grid: &Grid, north: u32, south: u32, first: u32, last: u32) -> Block {
        Block::new(
            RowBand::new(row_of(grid, north), row_of(grid, south)),
            col_of(grid, first),
            col_of(grid, last),
        )
    }

    /// The furthest cell of a block from its probe, by the distance test and nothing else.
    fn furthest_km(grid: &Grid, block: Block) -> f64 {
        let (probe_row, probe_col) = block.probe(grid);
        let from = grid.centre_of(probe_row, probe_col);
        cells_of(block)
            .into_iter()
            .map(|(row, col)| {
                great_circle_km(from, grid.centre_of(row_of(grid, row), col_of(grid, col)))
            })
            .fold(0.0f64, f64::max)
    }

    #[test]
    fn the_slack_dominates_every_cell_of_every_block() {
        // Exhaustive over cells, which is the only form of this claim worth having: a bound checked at a
        // corner is a bound checked where the author expected the maximum to be.
        for grid in [grid(), equatorial_grid()] {
            for cells in [4u32, 5, 18] {
                for block in Block::tile(&grid, spacing(cells)) {
                    let slack = slack_km(&grid, block);
                    let (probe_row, probe_col) = block.probe(&grid);
                    let from = grid.centre_of(probe_row, probe_col);

                    for (row, col) in cells_of(block) {
                        let to = grid.centre_of(row_of(&grid, row), col_of(&grid, col));
                        let actual = great_circle_km(from, to);
                        assert!(
                            slack >= actual,
                            "spacing {cells}, block {block:?}, cell {:?}: slack {slack} is under \
                             {actual}",
                            (row, col)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_single_cell_block_has_no_slack() {
        // Exactly zero rather than nearly: both offsets are zero, so the margin has nothing to inflate
        // and the widened radius a search builds from it is the radius itself.
        let grid = grid();
        for block in Block::tile(&grid, spacing(1)) {
            assert_eq!(slack_km(&grid, block), 0.0);
        }
    }

    #[test]
    fn a_one_column_block_is_the_meridian_arc_and_holds_only_by_the_margin() {
        // The first zero-margin configuration: with no longitude offset the two-hop path *is* the
        // meridian geodesic, so the bound is tight and the inequality survives on the inflation alone.
        // Tightness is asserted as well as the inequality — that is what says this case still exercises
        // the margin, rather than having drifted into a regime where slack to spare carries it.
        let grid = grid();
        for column in [0u32, 17, 35] {
            for (north, south) in [(0u32, 5u32), (6, 11), (12, 17), (0, 17)] {
                let block = block_of(&grid, north, south, column, column);
                let slack = slack_km(&grid, block);
                let furthest = furthest_km(&grid, block);

                assert!(slack >= furthest, "{block:?}: {slack} under {furthest}");
                assert!(
                    (slack - furthest) / furthest < 1e-12,
                    "{block:?}: the bound is no longer tight here, {slack} against {furthest}"
                );
            }
        }
    }

    #[test]
    fn a_one_row_block_on_the_equator_is_the_parallel_and_holds_only_by_the_margin() {
        // The second: on the equator the parallel is a great circle, so a longitude-only offset is again
        // exactly the geodesic. Row 4 of the equatorial fixture is latitude 0.
        let grid = equatorial_grid();
        assert_eq!(grid.centre_lat(row_of(&grid, 4)), 0.0);

        for (first, last) in [(0u32, 3u32), (10, 17), (0, 17)] {
            let block = block_of(&grid, 4, 4, first, last);
            let slack = slack_km(&grid, block);
            let furthest = furthest_km(&grid, block);

            assert!(slack >= furthest, "{block:?}: {slack} under {furthest}");
            assert!(
                (slack - furthest) / furthest < 1e-12,
                "{block:?}: the bound is no longer tight here, {slack} against {furthest}"
            );
        }
    }

    #[test]
    fn the_slack_shrinks_toward_the_pole_for_the_same_index_extent() {
        // What the probe's own cosine buys, and the reason the factor is not the block's worst latitude:
        // the same rectangle of indices bounds a far shorter distance beside the pole than at the
        // equator. Asserted as an ordering rather than a figure.
        let grid = equatorial_grid();
        let equator = slack_km(&grid, block_of(&grid, 4, 4, 0, 8));
        let polar = slack_km(&grid, block_of(&grid, 0, 0, 0, 8));
        assert!(polar < equator, "{polar} is not under {equator}");
    }

    proptest! {
        /// The same claim as the exhaustive test over a domain nobody chose: any rectangle of either
        /// fixture. Single-column and single-row blocks are inside it, so are blocks wider than half the
        /// grid, and so are row 0 and the row beside the southern edge — the configurations where the
        /// bound is tight or the geometry is worst are drawn rather than listed.
        #[test]
        fn the_slack_dominates_over_any_rectangle(
            equatorial in proptest::bool::ANY,
            north in 0u32..18,
            height in 1u32..18,
            first in 0u32..WIDTH,
            width in 1u32..WIDTH,
        ) {
            let grid = if equatorial { equatorial_grid() } else { grid() };
            let north = north.min(grid.height() - 1);
            let south = (north + height - 1).min(grid.height() - 1);
            let last = (first + width - 1).min(WIDTH - 1);

            let block = block_of(&grid, north, south, first, last);
            let slack = slack_km(&grid, block);
            prop_assert!(
                slack >= furthest_km(&grid, block),
                "{:?}: slack {} under {}",
                block,
                slack,
                furthest_km(&grid, block)
            );
        }
    }

    /// Closing in longitude, thirty degrees of latitude: `kernel.rs:539`'s shape, where a cap is clipped
    /// at the grid's edge rather than refused. It is here to exercise the clipping step of the bound's
    /// argument below.
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

    fn radius(km: f64) -> RadiusKm {
        RadiusKm::new(km).expect("a fixture radius is a length")
    }

    /// The padded payload a real build emits over a grid whose cells are distinct small integers, so a
    /// cell counted twice or not at all moves a total and every partial sum stays exact in f64.
    fn payload_over(grid: &Grid) -> Vec<f64> {
        let rows: Vec<Vec<f32>> = (0..grid.height())
            .map(|row| {
                (0..grid.width())
                    .map(|col| (row * grid.width() + col + 1) as f32)
                    .collect()
            })
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

    #[test]
    fn no_centre_in_a_block_beats_its_probes_widened_circle() {
        // Box 1 of issue #6, and the claim the whole search rests on. It is separate from the slack's own
        // test rather than folded into it so that a failure localises: that one is a claim about distance,
        // this one adds containment and the monotonicity #5's proptest pinned, and one test covering all
        // three would not say which had moved.
        //
        // The band grid is here for the step the two-hop proof does not cover. `Kernel` clips a cap at a
        // grid's northern or southern edge rather than refusing it, so on that grid both circles in the
        // inequality are intersections with the grid — and intersecting both sides of a containment with
        // the same set preserves it. Which is why the bound holds on every grid `Kernel` accepts, and not
        // only on a whole globe.
        for (grid, spacings) in [(grid(), vec![4u32, 18]), (band_grid(), vec![90u32])] {
            let payload = payload_over(&grid);
            let table = Table::new(grid, &payload).expect("the build emits the padded product");

            for radius_km in [1500.0, 8000.0] {
                // One kernel per row, which is the reuse the type exists for and what keeps this
                // exhaustive test to seconds rather than minutes.
                let row_kernels: Vec<Kernel> = grid
                    .rows()
                    .map(|row| {
                        Kernel::new(grid, row, radius(radius_km)).expect("a grid that closes")
                    })
                    .collect();

                for cells in &spacings {
                    for block in Block::tile(&grid, spacing(*cells)) {
                        let (probe_row, probe_col) = block.probe(&grid);
                        let widened = radius(radius_km + slack_km(&grid, block));
                        let bound_kernel =
                            Kernel::new(grid, probe_row, widened).expect("a grid that closes");
                        let bound = circle::population(&table, &bound_kernel, probe_col);

                        for (row, col) in cells_of(block) {
                            let here = circle::population(
                                &table,
                                &row_kernels[row as usize],
                                col_of(&grid, col),
                            );
                            assert!(
                                here <= bound,
                                "{}x{} grid, spacing {cells}, radius {radius_km} km, block \
                                 {block:?}, cell {:?}: {here} beats the bound {bound}",
                                grid.width(),
                                grid.height(),
                                (row, col)
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_better_candidate_is_the_same_whichever_order_they_arrive_in() {
        // Folded forwards and backwards over each list, because order-independence is the property and a
        // rule that took the first maximum it saw would pass one direction and fail the other.
        let grid = grid();
        let at = |row: u32, col: u32, population: f64| Candidate {
            row: row_of(&grid, row),
            col: col_of(&grid, col),
            population,
        };

        let cases = [
            // One clear maximum, which is the case that says the rule reads population first.
            (
                vec![at(3, 4, 10.0), at(0, 0, 50.0), at(17, 35, 20.0)],
                (0, 0),
            ),
            // Two equal maxima in different rows: the northern wins.
            (
                vec![at(9, 2, 50.0), at(2, 30, 50.0), at(5, 5, 10.0)],
                (2, 30),
            ),
            // Two equal maxima in one row: the western wins, by column index and not by longitude.
            (vec![at(4, 30, 50.0), at(4, 7, 50.0)], (4, 7)),
            // Everything tied, which is the all-zero raster in miniature.
            (vec![at(1, 1, 7.0), at(0, 35, 7.0), at(0, 3, 7.0)], (0, 3)),
        ];

        for (candidates, expected) in cases {
            for reversed in [false, true] {
                let mut order = candidates.clone();
                if reversed {
                    order.reverse();
                }
                let winner = order
                    .into_iter()
                    .reduce(Candidate::better)
                    .expect("the list is not empty");
                assert_eq!(
                    (winner.row.get(), winner.col.get()),
                    expected,
                    "reversed: {reversed}"
                );
            }
        }
    }

    #[test]
    fn a_block_given_its_columns_backwards_is_the_same_block() {
        let grid = grid();
        let rows = RowBand::new(row_of(&grid, 2), row_of(&grid, 5));
        assert_eq!(
            Block::new(rows, col_of(&grid, 30), col_of(&grid, 7)),
            Block::new(rows, col_of(&grid, 7), col_of(&grid, 30))
        );
    }
}
