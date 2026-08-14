// Step 3 of application.md "Approach": the most populous circle of a fixed ground radius, as a branch
// and bound over a refining grid of candidate centres.
//
// The multiresolution grid the technique needs is the grid of *centres*, not a second summation table: a
// rectangle of candidate cells carries one bound for every centre in it, and a rectangle that survives
// halves. So what is here is the rectangle, the slack it spans, and the level loop over the two — the
// circles themselves are `circle::population`'s and the geometry is `kernel`'s.

use std::num::NonZeroU32;

use crate::grid::{Col, Grid, Row};
use crate::table::RowBand;

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

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this narrow
// exemption; docs/ai/code.md allows both in tests.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::geodesy::LatLon;

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
