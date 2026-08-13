// The one test that reads the dataset itself, and the only place the reader is exercised on 21600
// LZW strips rather than on bytes this repository wrote. expect is what a test documents an invariant
// with; docs/ai/code.md allows it here.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use popcircles::geodesy::LatLon;
use popcircles::grid::Grid;
use popcircles::raster::geotiff::GeoTiffSource;
use popcircles::raster::{PixelType, RasterSource, RasterSpec};

#[test]
#[ignore = "reads the registry raster from data/; run `mise run test:raster`"]
fn the_registry_raster_reproduces_every_figure_the_registry_records() {
    // Literals rather than anything parsed, because #8 owns the way a user supplies a spec. Every one
    // is the `data/population/gpw-v4-11-unwpp-adjusted-count-2020-30arcsec.tif` row of
    // `data/README.md`; the sentinel is that row's nodata value, which is two ulps off -f32::MAX.
    let grid = Grid::new(
        43200,
        21600,
        LatLon {
            lat: 90.0,
            lon: -180.0,
        },
        1.0 / 120.0,
        -1.0 / 120.0,
    )
    .expect("the registry row describes a valid grid");
    let spec = RasterSpec {
        grid,
        epsg: 4326,
        pixel: PixelType::Float32,
        nodata: -3.402_823e38,
    };

    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "data",
        "population",
        "gpw-v4-11-unwpp-adjusted-count-2020-30arcsec.tif",
    ]
    .iter()
    .collect();
    let mut source = GeoTiffSource::open(&path, &spec).expect(
        "the registry raster is the raster the registry describes; run `mise run data:pull` first",
    );

    // f64 throughout, per application.md "Precision": every f32 widens exactly, so the only error is in
    // the additions — and there are 933 120 000 of them into a running total of 7.8e9, where one ulp is
    // 1.9e-6. Measured on this dataset, a naive accumulator lands 0.149 low, which is fifteen times the
    // 0.01 of a person the registry quotes. So the accumulator compensates (Neumaier), and the figure
    // becomes an assertion about the raster rather than about summation order.
    let mut total = 0.0f64;
    let mut lost = 0.0f64;
    let mut largest = 0.0f32;
    let mut rows = 0u32;
    while let Some(row) = source.next_row() {
        let row = row.expect("every strip of the registry raster decodes");
        assert_eq!(row.row.get(), rows, "rows arrive in order, none skipped");
        assert_eq!(row.values.len(), 43200);
        rows += 1;
        for value in row.values {
            let term = f64::from(*value);
            let sum = total + term;
            lost += if total.abs() >= term.abs() {
                (total - sum) + term
            } else {
                (term - sum) + total
            };
            total = sum;
            largest = largest.max(*value);
        }
    }
    let total = total + lost;
    assert_eq!(rows, 21600);

    let tallies = source.finish();
    assert_eq!(tallies.total(), 933_120_000);
    assert_eq!(tallies.populated, 182_358_616);
    assert_eq!(tallies.zero, 40_311_312);
    assert_eq!(tallies.populated + tallies.zero, 222_669_928, "land cells");
    assert_eq!(tallies.nodata, 933_120_000 - 222_669_928);
    // What turns "a non-sentinel negative is zero population with its own tally" from a policy into a
    // fact about this dataset: there are none.
    assert_eq!(tallies.unexpected_negative, 0);

    assert!(
        (total - 7_757_982_599.32).abs() < 0.01,
        "world total {total} against the registry's 7757982599.32"
    );
    // Widened to f64 so the registry's own digits can appear here: 602 380.375 is exact in f32, but
    // its shortest round-trip form is 602380.4, which is what the source would otherwise have to say.
    assert_eq!(f64::from(largest).to_bits(), 602_380.375_f64.to_bits());
}
