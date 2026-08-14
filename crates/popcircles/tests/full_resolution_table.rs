// ADR 0003 decision 2's tolerance, at the shape it was measured at: 21600 x 43200 cells, against an
// exact reference in i128 units of 2^-40. Neither table is ever held — the reference streams beside the
// build one padded row at a time, and the rectangles are taken between a handful of rows kept as they
// went past. expect and unwrap are what a test documents an invariant with; docs/ai/code.md allows both
// here. The three cast lints are allowed because each cast below is exact and says why at its site.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]

use std::convert::Infallible;

use popcircles::geodesy::LatLon;
use popcircles::grid::Grid;
use popcircles::raster::{CellTallies, RasterError, RasterRow, RasterSource, sanitise_row};
use popcircles::table::{Decimation, build};

const WIDTH: u32 = 43200;
const HEIGHT: u32 = 21600;
const NODATA: f32 = -3.402_823e38;

/// The reference's unit. Every generated cell has an exponent of at least 1, so its value is a whole
/// multiple of 2^-22 and scaling by this is exact; and a f64 rounded from a sum of multiples of 2^-40 is
/// itself one, because either the rounding grid is coarser than 2^-40 or the sum needed no rounding.
/// That is what makes the i128 side a reference rather than a second approximation.
const SCALE: f64 = (1u64 << 40) as f64;

/// f32 stores this many significand bits, and a generated cell fills all of them.
const SIGNIFICAND_BITS: u32 = 23;
const SIGNIFICAND_MASK: u32 = (1 << SIGNIFICAND_BITS) - 1;

/// The smallest exponent field a generated cell carries, which holds every one at or above 2.0 — the
/// property [`SCALE`] rests on — and the number of binades they spread over above it. Eighteen reaches
/// 2^20, against the registry raster's largest cell of 602 380.
const EXPONENT_FLOOR: u32 = 128;
const BINADES: u32 = 18;

/// The populated share, in thousandths. The registry raster's is about a fifth.
const POPULATED_PER_MILLE: u64 = 195;

/// splitmix64, so a cell depends on nothing but its position.
fn mix(value: u64) -> u64 {
    let mut z = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A cell of the raster, from its position alone, so the reference can regenerate the row the source
/// produced instead of sharing a buffer with it: nothing the build touched can reach what it is
/// checked against.
///
/// The exponent is geometric, so a cell of a million is about as rare as one is in the registry raster,
/// and the significand is full, so no cell is a round number the arithmetic could get right by luck.
fn cell(row: u32, col: u32) -> f32 {
    let hash = mix(u64::from(row) * u64::from(WIDTH) + u64::from(col));
    if hash % 1_000 >= POPULATED_PER_MILLE {
        return 0.0;
    }
    // Trailing zeros are geometric, and the shift is past the bits the populated share just read.
    let exponent = EXPONENT_FLOOR + (hash >> 10).trailing_zeros().min(BINADES);
    // The mask keeps 23 bits, so the narrowing is exact.
    let significand = ((hash >> 32) as u32) & SIGNIFICAND_MASK;
    f32::from_bits((exponent << SIGNIFICAND_BITS) | significand)
}

/// Exact, per [`SCALE`]: the product is integral and needs 73 bits at the largest total here, which
/// i128 holds and f64 scaling by a power of two never disturbed.
fn units(value: f64) -> i128 {
    (value * SCALE) as i128
}

/// One ulp at `value`'s magnitude.
fn ulp(value: f64) -> f64 {
    value.abs().next_up() - value.abs()
}

struct Generated {
    grid: Grid,
    next: u32,
    current: Vec<f32>,
    tallies: CellTallies,
}

impl RasterSource for Generated {
    fn grid(&self) -> Grid {
        self.grid
    }

    fn next_row(&mut self) -> Option<Result<RasterRow<'_>, RasterError>> {
        let index = self.next;
        let row = self.grid.row(index)?;
        self.current.clear();
        self.current.extend((0..WIDTH).map(|col| cell(index, col)));
        // Through the seam's own sanitiser rather than around it: what a consumer relies on is that no
        // sentinel reaches it, and these cells earn that the way a decoded strip does. It is the
        // identity on them — nothing here generates a sentinel or a negative — which is why the
        // reference can regenerate a row without repeating it.
        sanitise_row(&mut self.current, NODATA, &mut self.tallies);
        self.next += 1;
        Some(Ok(RasterRow {
            row,
            values: &self.current,
        }))
    }

    fn finish(self) -> CellTallies {
        self.tallies
    }
}

/// A padded row kept as it went past, so a rectangle can be taken between any two of them.
struct Kept {
    built: Vec<f64>,
    exact: Vec<i128>,
}

struct Reference {
    /// The exact padded row over every grid row folded so far.
    exact: Vec<i128>,
    /// How many padded rows have arrived. The next one carries grid row `padded - 1`.
    padded: u32,
    /// The worst absolute error seen, in the reference's own units. Absolute rather than in ulps
    /// because the unit is fixed and known only at the end; the assertions divide once.
    worst_cell_units: i128,
    keep_at: Vec<u32>,
    kept: Vec<Kept>,
}

impl Reference {
    fn take(&mut self, emitted: &[f64]) {
        if self.padded > 0 {
            let row = self.padded - 1;
            let mut prefix = 0i128;
            for col in 0..WIDTH {
                prefix += units(f64::from(cell(row, col)));
                self.exact[col as usize + 1] += prefix;
            }
        }

        for (index, &value) in emitted.iter().enumerate() {
            let dropped = (units(value) - self.exact[index]).abs();
            self.worst_cell_units = self.worst_cell_units.max(dropped);
        }

        if self.keep_at.contains(&self.padded) {
            self.kept.push(Kept {
                built: emitted.to_vec(),
                exact: self.exact.clone(),
            });
        }
        self.padded += 1;
    }
}

/// The four-corner subtraction. `table.rs` owns the traversal and its own tests pin it; what this
/// measures is the arithmetic, which is why it works on kept corners rather than on a `Table` the whole
/// 7.5 GB payload would have to exist for.
fn corners(row: &[f64], other: &[f64], first: u32, last: u32) -> f64 {
    other[last as usize] - row[last as usize] - other[first as usize] + row[first as usize]
}

fn corners_exact(row: &[i128], other: &[i128], first: u32, last: u32) -> i128 {
    other[last as usize] - row[last as usize] - other[first as usize] + row[first as usize]
}

#[test]
#[ignore = "builds a 933 120 000 cell table; run `mise run test:slow`"]
fn the_full_resolution_table_stays_inside_decision_2s_tolerance() {
    let grid = Grid::new(
        WIDTH,
        HEIGHT,
        LatLon {
            lat: 90.0,
            lon: -180.0,
        },
        1.0 / 120.0,
        -1.0 / 120.0,
    )
    .expect("the registry raster's shape is a valid grid");

    // Nine padded rows spread over the grid, both edges included, at about 1 MB each.
    let keep_at: Vec<u32> = (0..=8).map(|step| step * HEIGHT / 8).collect();
    let mut reference = Reference {
        exact: vec![0i128; WIDTH as usize + 1],
        padded: 0,
        worst_cell_units: 0,
        keep_at,
        kept: Vec::new(),
    };
    let source = Generated {
        grid,
        next: 0,
        current: Vec::with_capacity(WIDTH as usize),
        tallies: CellTallies::default(),
    };

    let built = build(source, Decimation::none(grid), &mut (), |row| {
        reference.take(row);
        Ok::<(), Infallible>(())
    })
    .expect("a generated source cannot fail and neither can this sink");

    assert_eq!(built.tallies.total(), u64::from(WIDTH) * u64::from(HEIGHT));
    assert_eq!(reference.padded, HEIGHT + 1);

    // Wrapped, contiguous and full-width spans between every pair of kept rows. The spans come from
    // the same hash the cells do, so the set is fixed: a tolerance that holds only on some runs is not
    // a tolerance.
    let mut worst_query_units = 0i128;
    let mut queries = 0u32;
    for (index, north) in reference.kept.iter().enumerate() {
        for south in &reference.kept[index + 1..] {
            for step in 0..300u64 {
                let west = (mix(step * 2) % u64::from(WIDTH)) as u32;
                let east = (mix(step * 2 + 1) % u64::from(WIDTH)) as u32;
                let (value, exact) = if west <= east {
                    (
                        corners(&north.built, &south.built, west, east + 1),
                        corners_exact(&north.exact, &south.exact, west, east + 1),
                    )
                } else {
                    (
                        corners(&north.built, &south.built, west, WIDTH)
                            + corners(&north.built, &south.built, 0, east + 1),
                        corners_exact(&north.exact, &south.exact, west, WIDTH)
                            + corners_exact(&north.exact, &south.exact, 0, east + 1),
                    )
                };
                worst_query_units = worst_query_units.max((units(value) - exact).abs());
                queries += 1;
            }

            let value = corners(&north.built, &south.built, 0, WIDTH);
            let exact = corners_exact(&north.exact, &south.exact, 0, WIDTH);
            worst_query_units = worst_query_units.max((units(value) - exact).abs());
            queries += 1;
        }
    }

    // The unit both figures are in, and the same one decision 2's measurements are in: one ulp at the
    // table's own magnitude. Not the ulp of each result — a rectangle is a difference of four corners
    // each of the table's magnitude, so a narrow rectangle carries the corners' rounding rather than
    // its own, and holding it to the ulp of its smaller answer would be a claim about cancellation
    // instead of about the build.
    let unit = ulp(built.total) * SCALE;
    let cell_ulps = reference.worst_cell_units as f64 / unit;
    let query_ulps = worst_query_units as f64 / unit;

    // Decision 2's two numbers, and assertions about the construction rather than about f64. ADR 0003
    // measured the uncompensated separable form at 1.2e-4 at this shape, 126 ulp; drop the error term
    // from `two_sum` and this test reports 105.5 ulp per cell, so it fails by two orders of magnitude
    // the moment the correction stops being applied. Compensated it measures 0.5 and 1.25. The query
    // budget is four rather than one because four corners are subtracted, each already rounded.
    assert!(
        cell_ulps <= 1.0,
        "worst cell error {cell_ulps} ulp over {} cells, total {}",
        u64::from(WIDTH) * u64::from(HEIGHT),
        built.total
    );
    assert!(
        query_ulps <= 4.0,
        "worst query error {query_ulps} ulp over {queries} rectangles"
    );
}
