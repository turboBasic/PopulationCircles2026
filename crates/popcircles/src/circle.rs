// The ground step 3 of application.md "Approach" stands on: the population inside a circle of a given
// ground radius, anywhere on the globe. The geometry is the kernel's and the arithmetic is the table's,
// so what is here is the fold between them and nothing else — no distance, no latitude, no angle.

use crate::grid::Col;
use crate::kernel::Kernel;
use crate::table::{RowBand, Table};

/// The population inside the circle `kernel` describes, centred on `centre` in the kernel's own row.
///
/// One rectangle per row the cap reaches, added in the order [`Kernel::place`] yields them, north to
/// south. The order is fixed rather than incidental: a rerun has to give the same answer down to the last
/// bit, so nothing here sorts the rows, splits them across threads, or merges two of them into one query.
///
/// A grid whose columns do not close needs no case: [`Kernel::new`] refuses one, so holding a kernel is
/// the proof, and the assertion below extends it to the table.
///
/// # Panics
/// If the kernel was built over a grid other than the table's. Both are a [`crate::grid::Grid`], so the
/// types cannot catch it, and it is a wiring mistake rather than an input — the same reason
/// [`crate::table::build`] stops on a decimation minted against a foreign grid. Also if `centre` was
/// minted by a larger grid; [`crate::grid::Col`] says why that is a stop.
#[must_use]
pub fn population(table: &Table<'_>, kernel: &Kernel, centre: Col) -> f64 {
    assert_eq!(
        *table.grid(),
        *kernel.grid(),
        "the kernel was built over a different grid than this table"
    );

    kernel
        .place(centre)
        .map(|(row, cols)| table.population(RowBand::new(row, row), cols))
        .sum()
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this narrow
// exemption; docs/ai/code.md allows both in tests. float_cmp is the point rather than a concession: every
// fixture cell is a small integer, so each side of an assertion below is an exact f64 and a tolerance
// would let a dropped row or a doubled one pass. cast_precision_loss likewise — the largest cell is 648,
// which u32 -> f32 holds exactly.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_precision_loss
)]
mod tests {
    use std::convert::Infallible;

    use super::*;
    use crate::geodesy::{LatLon, great_circle_km};
    use crate::grid::{Grid, Row};
    use crate::raster::Synthetic;
    use crate::table::{Decimation, build};

    const WIDTH: u32 = 36;
    const HEIGHT: u32 = 18;

    /// The registry raster's sentinel, so the fixture reaches the table by the path a real raster takes.
    const NODATA: f32 = -3.402_823e38;

    /// Ten degrees a side, closing in longitude: the smallest shape that has a kernel, a seam and a pole
    /// at once, and small enough that a whole-grid reference scan at every centre is a second of work.
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

    /// Distinct at every position, so a cell counted twice or not at all moves the total, and no larger
    /// than 648, so every partial sum of them is exact in f64 and the assertions are equalities.
    fn cell(row: u32, col: u32) -> f32 {
        (row * WIDTH + col + 1) as f32
    }

    fn cells() -> Vec<Vec<f32>> {
        (0..HEIGHT)
            .map(|row| (0..WIDTH).map(|col| cell(row, col)).collect())
            .collect()
    }

    /// The padded payload a real build emits over these rows, rather than one written out by hand: the
    /// fixture is then the path the search uses and not a second construction of it.
    fn payload_over(rows: Vec<Vec<f32>>) -> Vec<f64> {
        let grid = grid();
        let source = Synthetic::new(grid, NODATA, rows).expect("the rows are the grid's shape");
        let mut payload = Vec::new();
        build(source, Decimation::none(grid), &mut (), |row| {
            payload.extend_from_slice(row);
            Ok::<(), Infallible>(())
        })
        .expect("neither a synthetic source nor this sink can fail");
        payload
    }

    /// Every cell of the whole grid within `radius_km` of the centre cell, added up directly.
    ///
    /// The scan covers the grid and not the kernel's band, which is the only way it can see a cell the
    /// kernel left out; and it tests kilometres against kilometres, so it owes nothing to the angle the
    /// kernel converted the radius to.
    fn by_distance(grid: &Grid, centre: (Row, Col), radius_km: f64) -> f64 {
        let from = grid.centre_of(centre.0, centre.1);
        grid.rows()
            .flat_map(|row| grid.cols().map(move |col| (row, col)))
            .filter(|(row, col)| great_circle_km(from, grid.centre_of(*row, *col)) <= radius_km)
            .map(|(row, col)| f64::from(cell(row.get(), col.get())))
            .sum()
    }

    #[test]
    fn a_circles_population_is_the_sum_of_the_cells_a_distance_test_admits() {
        // Every cell of the fixture as a centre, at four radii: 1500 km spans a column or two, 20 016 km
        // is past half the circumference and closes every row, and the two between cover the pole from
        // one side and not the other. Exhaustive in longitude is what says the seam is not a special
        // case, and exhaustive in latitude is what says the pole is not either.
        let grid = grid();
        let payload = payload_over(cells());
        let table = Table::new(grid, &payload).expect("the build emits the padded product");

        for radius_km in [1500.0, 4000.0, 8000.0, 20_016.0] {
            for centre_row in grid.rows() {
                // One kernel per row, placed at all 36 columns, which is the reuse the type exists for.
                let kernel = Kernel::new(grid, centre_row, radius_km)
                    .expect("a whole-globe grid and a radius that is a length");
                for centre_col in grid.cols() {
                    assert_eq!(
                        population(&table, &kernel, centre_col),
                        by_distance(&grid, (centre_row, centre_col), radius_km),
                        "row {}, column {}, radius {radius_km} km",
                        centre_row.get(),
                        centre_col.get()
                    );
                }
            }
        }
    }

    #[test]
    #[should_panic(expected = "built over a different grid")]
    fn a_kernel_from_another_grid_is_no_circle_here() {
        let grid = grid();
        let coarser = Grid::new(
            WIDTH / 2,
            HEIGHT / 2,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            20.0,
            -20.0,
        )
        .expect("an 18 x 9 whole-globe grid is valid");
        let payload = payload_over(cells());
        let table = Table::new(grid, &payload).expect("the build emits the padded product");
        let kernel = Kernel::new(coarser, coarser.middle_row(), 3000.0)
            .expect("a whole-globe grid has kernels");

        let _ = population(
            &table,
            &kernel,
            grid.col(0).expect("a column of the fixture"),
        );
    }
}
