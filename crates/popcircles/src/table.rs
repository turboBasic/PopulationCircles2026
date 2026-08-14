// The computing half of the summation table. ADR 0003 decision 1 keeps the file, the header and
// everything that serialises in `table/cache.rs`, so nothing here can be reached without a grid and a
// slice.
use crate::grid::{Col, Grid, Row};
use crate::progress::Progress;
use crate::raster::{CellTallies, RasterError, RasterSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TableError {
    #[error("a padded table over this grid holds {expected} cells; the payload holds {found}")]
    PayloadLength { expected: usize, found: usize },
}

/// Why a build stopped. The sink's own failure stays its own type, so this module needs no vocabulary
/// for whatever a caller is emitting rows into.
#[derive(Debug, thiserror::Error)]
pub enum BuildError<E> {
    #[error("the raster could not be read")]
    Raster(#[from] RasterError),

    #[error("a completed table row could not be taken")]
    Sink(#[source] E),
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

/// What a build produced beside the rows it emitted, and everything a cache header needs of them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BuiltTable {
    /// The identity of the cells this table was built from — ADR 0003 decision 3, and the only thing
    /// that says two tables are the same table.
    pub digest: u64,
    pub tallies: CellTallies,
    /// The whole raster's population: the table's last cell, so it carries the compensation the rest
    /// of the table carries rather than being summed a second way.
    pub total: f64,
}

// Decision 3's digest in full, because a digest whose word width or order is left to the
// implementation is a number that happens to match today rather than an identity: FNV-1a, 64-bit,
// standard offset basis and prime, over each sanitised cell's `f32` bits widened to `u64`, in
// row-major order.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Streams a raster into the padded rows of a summation table, handing each row to `emit` as it is
/// completed.
///
/// What stays resident is one accumulator row, one correction row and the row the source lends — 864 KB
/// at the registry raster's width, for a table of 7.5 GB — so the build's memory is the grid's width
/// rather than its area. The accumulator holds the correctly rounded column sum and the correction
/// holds exactly what f64 could not, renormalised after every row; that is why the accumulator can be
/// emitted as it stands, where Neumaier's deferred correction would need a third row resident to add
/// the two together into.
///
/// # Errors
/// [`BuildError::Raster`] when the source fails mid-stream, [`BuildError::Sink`] when `emit` does.
pub fn build<S, P, F, E>(
    mut source: S,
    progress: &mut P,
    mut emit: F,
) -> Result<BuiltTable, BuildError<E>>
where
    S: RasterSource,
    P: Progress,
    F: FnMut(&[f64]) -> Result<(), E>,
{
    let grid = source.grid();
    let width = grid.width() as usize;
    let rows = u64::from(grid.height());

    let mut acc = vec![0.0f64; width + 1];
    let mut corr = vec![0.0f64; width + 1];
    let mut digest = FNV_OFFSET_BASIS;

    // The zero row, which is the padding itself: emitting it here is what lets a rectangle touching
    // the north edge subtract four corners like any other.
    emit(&acc).map_err(BuildError::Sink)?;
    progress.advance(0, rows);

    let mut done = 0u64;
    while let Some(row) = source.next_row() {
        let values = row?.values;

        // The row prefix lives in two scalars rather than a row of its own: each prefix is consumed by
        // the column it belongs to in the same step, so nothing here ever needs a whole prefix row.
        let mut prefix = 0.0f64;
        let mut prefix_corr = 0.0f64;
        for (index, &cell) in values.iter().enumerate() {
            digest = (digest ^ u64::from(cell.to_bits())).wrapping_mul(FNV_PRIME);

            // The one widening in the build, and the ground rule behind it: f32 -> f64 is exact, and
            // no accumulator below is ever anything but f64.
            let (sum, dropped) = two_sum(prefix, f64::from(cell));
            (prefix, prefix_corr) = two_sum(sum, prefix_corr + dropped);

            let column = index + 1;
            let (sum, dropped) = two_sum(acc[column], prefix);
            (acc[column], corr[column]) = two_sum(sum, corr[column] + dropped);
        }

        emit(&acc).map_err(BuildError::Sink)?;
        done += 1;
        progress.advance(done, rows);
    }

    Ok(BuiltTable {
        digest,
        tallies: source.finish(),
        total: acc[width],
    })
}

/// Knuth's two-sum: the rounded sum, and exactly what the rounding dropped, for any two magnitudes.
///
/// The unconditional form rather than Neumaier's comparison of magnitudes — the same arithmetic, and
/// the error term is what both are for, but this one costs no branch per cell over 933 120 000 of them.
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let sum = a + b;
    let b_rounded = sum - a;
    (sum, (a - (sum - b_rounded)) + (b - b_rounded))
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this
// narrow exemption; docs/ai/code.md allows both in tests. float_cmp joins them because every value
// below is a small integer, exact in f64: what these assertions pin is the indexing and the wrapping,
// so a tolerance would let an off-by-one in the padding pass. cast_precision_loss likewise — the
// generated cells are integers below 2^20, where u32 -> f32 is exact.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_precision_loss
)]
mod tests {
    use std::convert::Infallible;
    use std::ops::RangeInclusive;

    use proptest::prelude::*;

    use super::*;
    use crate::geodesy::LatLon;
    use crate::raster::Synthetic;

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

    /// The registry raster's sentinel, so the sanitising the digest depends on is the sanitising the
    /// real reader does.
    const NODATA: f32 = -3.402_823e38;

    fn built(grid: Grid, rows: Vec<Vec<f32>>) -> (Vec<f64>, BuiltTable) {
        let source = Synthetic::new(grid, NODATA, rows).expect("the rows are the grid's shape");
        let mut payload = Vec::new();
        let built = build(source, &mut (), |row| {
            payload.extend_from_slice(row);
            Ok::<(), Infallible>(())
        })
        .expect("neither a synthetic source nor this sink can fail");
        (payload, built)
    }

    #[test]
    fn the_digest_is_fnv_1a_over_the_sanitised_cells() {
        // Computed outside this crate, from the three cells the sanitised row holds: 1.0 is
        // 0x3f80_0000, the sentinel becomes 0.0 and so contributes 0x0000_0000, and 2.5 is
        // 0x4020_0000. Folding those three words into the offset basis with the FNV prime gives the
        // value below. A digest checked against whatever this build produces would pin nothing, which
        // is the whole reason decision 3 spells the definition out.
        let grid = Grid::new(
            3,
            1,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            120.0,
            -180.0,
        )
        .expect("a 3 x 1 whole-globe grid is valid");
        let (_, built) = built(grid, vec![vec![1.0, NODATA, 2.5]]);

        assert_eq!(built.digest, 0x3a5d_5e3b_082f_2fb7);
        assert_eq!(built.total, 3.5);
        assert_eq!(built.tallies.nodata, 1);
        assert_eq!(built.tallies.populated, 2);
    }

    proptest! {
        /// Exactly, not within a tolerance: the cells are integers below 2^20, so every partial sum of
        /// them is exact in f64 and any difference here is the traversal — an index, the padding
        /// offset, or a seam — rather than arithmetic. Decision 2's ulp budget is the other claim and
        /// is tested at full magnitude, where no direct sum exists to compare against.
        #[test]
        fn every_rectangle_matches_a_direct_sum(
            (width, height, cells) in (1u32..=5, 1u32..=4).prop_flat_map(|(width, height)| {
                (
                    Just(width),
                    Just(height),
                    prop::collection::vec(
                        prop::collection::vec(0u32..(1 << 20), width as usize),
                        height as usize,
                    ),
                )
            })
        ) {
            let grid = Grid::new(
                width,
                height,
                LatLon { lat: 90.0, lon: -180.0 },
                360.0 / f64::from(width),
                -180.0 / f64::from(height),
            )
            .expect("a whole-globe grid of any shape is valid");

            let values: Vec<Vec<f32>> = cells
                .iter()
                .map(|row| row.iter().map(|cell| *cell as f32).collect())
                .collect();
            let (payload, built) = built(grid, values.clone());
            let table = Table::new(grid, &payload).expect("the build emits the padded product");

            let direct = |rows: &[u32], cols: &[u32]| -> f64 {
                let mut total = 0.0;
                for row in rows {
                    for col in cols {
                        total += f64::from(values[*row as usize][*col as usize]);
                    }
                }
                total
            };

            let every_row: Vec<u32> = (0..height).collect();
            let every_col: Vec<u32> = (0..width).collect();
            prop_assert_eq!(built.total, direct(&every_row, &every_col));

            for north in 0..height {
                for south in north..height {
                    let rows: Vec<u32> = (north..=south).collect();
                    let band = RowBand::new(grid.row(north).unwrap(), grid.row(south).unwrap());
                    prop_assert_eq!(
                        table.population(band, ColSpan::FullTurn),
                        direct(&rows, &every_col)
                    );

                    for west in 0..width {
                        for east in 0..width {
                            // west > east is the wrapped case, and on a grid one column wide it is
                            // also the full width: both are ordinary spans here rather than cases a
                            // caller has to spot.
                            let cols: Vec<u32> = if west <= east {
                                (west..=east).collect()
                            } else {
                                (west..width).chain(0..=east).collect()
                            };
                            let span = ColSpan::Through {
                                west: grid.col(west).unwrap(),
                                east: grid.col(east).unwrap(),
                            };
                            prop_assert_eq!(table.population(band, span), direct(&rows, &cols));
                        }
                    }
                }
            }
        }
    }
}
