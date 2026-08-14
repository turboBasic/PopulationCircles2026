// The computing half of the summation table. ADR 0003 decision 1 keeps the file, the header and
// everything that serialises in `table/cache.rs`, so nothing here can be reached without a grid and a
// slice.
use crate::grid::{Col, Grid, Row};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TableError {
    #[error("a padded table over this grid holds {expected} cells; the payload holds {found}")]
    PayloadLength { expected: usize, found: usize },
}

/// A band of rows, inclusive at both ends.
///
/// Inclusive because a [`Row`] is only ever minted by the grid that contains it, so a half-open band
/// would need a southern end no grid will mint. Any pair of rows is a band, so the constructor orders
/// them rather than refusing one order: there is nothing here for a caller to get wrong and nothing
/// downstream to re-check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowBand {
    north: Row,
    south: Row,
}

impl RowBand {
    #[must_use]
    pub fn new(a: Row, b: Row) -> Self {
        Self {
            north: a.min(b),
            south: a.max(b),
        }
    }

    #[must_use]
    pub const fn north(self) -> Row {
        self.north
    }

    #[must_use]
    pub const fn south(self) -> Row {
        self.south
    }
}

/// The columns a query covers.
///
/// The full turn is a **variant**, not a `west == east` pair: on a grid whose columns close on
/// themselves, a span given as two indices cannot say whether it means one column or all of them, and
/// the answer a caller wants is the one it did not encode. Splitting the wrapped case into two
/// rectangles is then this module's business rather than every caller's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColSpan {
    /// `west` through `east` inclusive, running east past the last column and on from the first when
    /// `west > east`.
    Through { west: Col, east: Col },
    /// Every column of the grid, exactly once.
    FullTurn,
}

/// A summed-area table over a grid, borrowing its payload.
///
/// The payload is padded to `(height + 1) x (width + 1)` with a zero first row and column — ADR 0003
/// decision 4 — so element `(r, c)` holds the population of every cell strictly north of grid row `r`
/// and strictly west of grid column `c`, and a rectangle touching the north or west edge needs no
/// branch of its own.
///
/// It borrows a slice instead of taking a storage generic, which is the same decision: a `Vec` and a
/// mapping are both a slice by the time a query sees one, and the checked cast belongs where it can
/// fail rather than behind an accessor.
#[derive(Debug, Clone, Copy)]
pub struct Table<'a> {
    grid: Grid,
    cells: &'a [f64],
}

impl<'a> Table<'a> {
    /// # Errors
    /// [`TableError::PayloadLength`] when `cells` is not the padded product of the grid's dimensions.
    /// That is the whole of what a slice can be wrong about here, and checking it once is what lets
    /// every query below index without a bound of its own.
    pub fn new(grid: Grid, cells: &'a [f64]) -> Result<Self, TableError> {
        let expected = padded_len(&grid);
        if cells.len() != expected {
            return Err(TableError::PayloadLength {
                expected,
                found: cells.len(),
            });
        }
        Ok(Self { grid, cells })
    }

    #[must_use]
    pub const fn grid(&self) -> &Grid {
        &self.grid
    }

    /// The population of a rectangle: four lookups, and two of those for a span that wraps.
    ///
    /// # Panics
    /// If an index was minted by a larger grid; [`Row`] says why that is a stop.
    #[must_use]
    pub fn population(&self, rows: RowBand, cols: ColSpan) -> f64 {
        let width = self.grid.width();
        // The band is ordered, so its southern end is the only one that can run off the grid.
        assert!(
            rows.south.get() < self.grid.height(),
            "row {} is not a row of a {}-row table",
            rows.south.get(),
            self.grid.height()
        );

        match cols {
            // One rectangle over every column, never two pieces meeting at a seam. The full turn is
            // the case a split double-counts, and here there is no split to get wrong.
            ColSpan::FullTurn => self.rectangle(rows, 0, width),
            ColSpan::Through { west, east } => {
                assert!(
                    west.get() < width && east.get() < width,
                    "({}, {}) are not columns of a {}-column table",
                    west.get(),
                    east.get(),
                    width
                );
                if west.get() <= east.get() {
                    self.rectangle(rows, west.get(), east.get() + 1)
                } else {
                    self.rectangle(rows, west.get(), width)
                        + self.rectangle(rows, 0, east.get() + 1)
                }
            }
        }
    }

    /// `first` and `last` are grid columns, half-open, and both are already padded indices.
    fn rectangle(&self, rows: RowBand, first: u32, last: u32) -> f64 {
        let north = rows.north.get();
        let south = rows.south.get() + 1;
        self.corner(south, last) - self.corner(north, last) - self.corner(south, first)
            + self.corner(north, first)
    }

    fn corner(&self, row: u32, col: u32) -> f64 {
        let stride = self.grid.width() as usize + 1;
        self.cells[row as usize * stride + col as usize]
    }
}

/// Saturating rather than wrapping, so a grid too large for this host's addresses is reported as a
/// payload of the wrong length instead of as a smaller shape some payload happens to fit.
fn padded_len(grid: &Grid) -> usize {
    let cells = (u64::from(grid.width()) + 1) * (u64::from(grid.height()) + 1);
    usize::try_from(cells).unwrap_or(usize::MAX)
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this
// narrow exemption; docs/ai/code.md allows both in tests. float_cmp joins them because every value
// below is a small integer, exact in f64: what these assertions pin is the indexing and the wrapping,
// so a tolerance would let an off-by-one in the padding pass.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use std::ops::RangeInclusive;

    use super::*;
    use crate::geodesy::LatLon;

    // The cells and the padded table over them are both written out, rather than one derived from the
    // other: the code that derives one is 1.3's builder, and a fixture it computed would assert
    // nothing about the indexing here.
    static CELLS: [[f64; 4]; 3] = [
        [1.0, 2.0, 3.0, 4.0],
        [5.0, 6.0, 7.0, 8.0],
        [9.0, 10.0, 11.0, 12.0],
    ];

    static PAYLOAD: [[f64; 5]; 4] = [
        [0.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 3.0, 6.0, 10.0],
        [0.0, 6.0, 14.0, 24.0, 36.0],
        [0.0, 15.0, 33.0, 54.0, 78.0],
    ];

    fn grid() -> Grid {
        // Four columns of 90 degrees close on themselves, which is what makes the wrapped and
        // full-turn spans below cases this grid actually has.
        Grid::new(
            4,
            3,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            90.0,
            -60.0,
        )
        .expect("a 4 x 3 whole-globe grid is valid")
    }

    fn table() -> Table<'static> {
        Table::new(grid(), PAYLOAD.as_flattened()).expect("the payload is the padded product")
    }

    fn band(north: u32, south: u32) -> RowBand {
        let grid = grid();
        RowBand::new(
            grid.row(north).expect("a row of the fixture"),
            grid.row(south).expect("a row of the fixture"),
        )
    }

    fn through(west: u32, east: u32) -> ColSpan {
        let grid = grid();
        ColSpan::Through {
            west: grid.col(west).expect("a column of the fixture"),
            east: grid.col(east).expect("a column of the fixture"),
        }
    }

    /// The reference: the named cells added up directly. `cols` is a list rather than a range so a
    /// wrapped span's reference is as easy to write as a contiguous one's.
    fn direct(rows: RangeInclusive<usize>, cols: &[usize]) -> f64 {
        rows.flat_map(|row| cols.iter().map(move |col| CELLS[row][*col]))
            .sum()
    }

    #[test]
    fn an_interior_rectangle_is_the_sum_of_its_cells() {
        assert_eq!(
            table().population(band(1, 2), through(1, 2)),
            direct(1..=2, &[1, 2])
        );
    }

    #[test]
    fn a_span_wrapped_across_the_antimeridian_covers_both_pieces() {
        assert_eq!(
            table().population(band(0, 1), through(3, 0)),
            direct(0..=1, &[3, 0])
        );
    }

    #[test]
    fn the_full_turn_is_the_row_band_itself() {
        // Against the band's own total, not against two rectangles: a full turn assembled from a pair
        // of spans is the double count this enum exists to make unwritable, and comparing one
        // assembly with another would agree with it.
        assert_eq!(
            table().population(band(1, 2), ColSpan::FullTurn),
            direct(1..=2, &[0, 1, 2, 3])
        );
    }

    #[test]
    fn a_span_of_one_column_is_one_column() {
        // The other half of that: `west == east` says one column and cannot be read as all of them.
        assert_eq!(
            table().population(band(0, 2), through(2, 2)),
            direct(0..=2, &[2])
        );
    }

    #[test]
    fn a_payload_that_is_not_the_padded_product_is_refused() {
        let full = PAYLOAD.as_flattened();
        let short = &full[..full.len() - 1];
        let long: Vec<f64> = full.iter().copied().chain([0.0]).collect();

        assert_eq!(
            Table::new(grid(), short).unwrap_err(),
            TableError::PayloadLength {
                expected: 20,
                found: 19
            }
        );
        assert_eq!(
            Table::new(grid(), &long).unwrap_err(),
            TableError::PayloadLength {
                expected: 20,
                found: 21
            }
        );
    }
}
