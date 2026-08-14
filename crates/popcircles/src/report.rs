// The published shape of a result, and the only place in this crate a serde derive appears. ADR 0001
// decision 3: the domain types change when the search changes, so what is serialised is a separate
// representation with its own version, and a field here is a promise to two renderers and two
// command surfaces.
use std::path::Path;

use serde::Serialize;

use crate::geodesy::{LatLon, wrap_lon};
use crate::grid::Grid;
use crate::raster::CellTallies;
use crate::table::cache::Identity;
use crate::table::{BuiltTable, ColSpan, RowBand, Window};

/// Bumped when a change to a document below is not additive — a renamed or removed field, or one
/// whose meaning moved. A new field does not bump it.
pub const SCHEMA_VERSION: u32 = 1;

/// Every document the program writes.
///
/// `schema_version` is declared first because serde emits struct fields in declaration order, so a
/// consumer reads the version before anything it might not understand. `provenance` precedes `result`
/// for the same reason one step out: what produced a document is read before the document.
#[derive(Debug, Clone, Serialize)]
pub struct Envelope<T> {
    schema_version: u32,
    tool: &'static str,
    tool_version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    provenance: Option<Provenance>,
    result: T,
}

impl<T> Envelope<T> {
    /// A document with no provenance to declare, which is every command that reads no cached table.
    #[must_use]
    pub const fn new(result: T) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            // This crate, not whichever binary is writing: the format is the library's, and stamping
            // the caller's name would make one document's producer unidentifiable from another's.
            tool: env!("CARGO_PKG_NAME"),
            tool_version: env!("CARGO_PKG_VERSION"),
            provenance: None,
            result,
        }
    }

    /// The fields are spelled out rather than updated over [`Self::new`]: a functional update would drop
    /// the `None` it replaces, and dropping a value with glue is not something a `const fn` may do.
    #[must_use]
    pub const fn with_provenance(result: T, provenance: Provenance) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            tool: env!("CARGO_PKG_NAME"),
            tool_version: env!("CARGO_PKG_VERSION"),
            provenance: Some(provenance),
            result,
        }
    }
}

/// The table a command answered from, and where it sits.
///
/// Two of the three facts here are the cache's own and the third is not, which is why the distinction is
/// documented rather than left to a reader: `digest` and `decimation` are what a cache **attested** to,
/// because opening one compares both. `grid` is the grid the caller **declared** — the header binds a
/// width, a height and a factor and no origin or step, so a table built over one geometry opens cleanly
/// for a query declaring another. `FU-11` is that gap; closing it is a record's call.
#[derive(Debug, Clone, Serialize)]
pub struct Provenance {
    digest: String,
    decimation: u32,
    grid: GridSummary,
    cache: CacheFiles,
}

impl Provenance {
    #[must_use]
    pub fn new(identity: &Identity, header: &Path, payload: &Path) -> Self {
        Self {
            digest: hexadecimal(identity.digest),
            decimation: identity.decimation.factor(),
            grid: GridSummary::from(identity.decimation.grid()),
            cache: CacheFiles::new(header, payload),
        }
    }
}

/// A coordinate as published. Longitude is reduced here, which is the reduction
/// [`Grid::centre_of`] leaves to whatever presents its result.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Coordinate {
    lat: f64,
    lon: f64,
}

impl From<LatLon> for Coordinate {
    fn from(at: LatLon) -> Self {
        Self {
            lat: at.lat,
            lon: wrap_lon(at.lon),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct DistanceReport {
    from: Coordinate,
    to: Coordinate,
    great_circle_km: f64,
}

impl DistanceReport {
    #[must_use]
    pub fn new(from: LatLon, to: LatLon, great_circle_km: f64) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            great_circle_km,
        }
    }
}

/// What a grid is, for a caller that has to decide whether it is the grid it meant. The cell area is
/// the middle row's because area varies by row: one figure stands for the grid's scale only if it
/// says which row it came from, and the middle is the row furthest from both degenerate ends.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct GridSummary {
    width: u32,
    height: u32,
    origin: Coordinate,
    lon_step: f64,
    lat_step: f64,
    spans_full_turn: bool,
    middle_row_cell_area_km2: f64,
}

impl From<&Grid> for GridSummary {
    fn from(grid: &Grid) -> Self {
        Self {
            width: grid.width(),
            height: grid.height(),
            origin: grid.origin().into(),
            lon_step: grid.lon_step(),
            lat_step: grid.lat_step(),
            spans_full_turn: grid.spans_full_turn(),
            middle_row_cell_area_km2: grid.cell_area_km2(grid.middle_row()),
        }
    }
}

/// Where every cell of a drained raster went, as published.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct CellTalliesReport {
    nodata: u64,
    unexpected_negative: u64,
    zero: u64,
    populated: u64,
    total: u64,
}

impl From<CellTallies> for CellTalliesReport {
    fn from(tallies: CellTallies) -> Self {
        Self {
            nodata: tallies.nodata,
            unexpected_negative: tallies.unexpected_negative,
            zero: tallies.zero,
            populated: tallies.populated,
            total: tallies.total(),
        }
    }
}

/// What a summation table build settled, and where it published the table.
///
/// The digest is a string of hexadecimal rather than a number: it is an identity and not a quantity, and
/// a `u64` past 2^53 does not survive a JSON consumer that parses numbers as doubles. `digest` is what a
/// later query passes back to name the table it wants.
#[derive(Debug, Clone, Serialize)]
pub struct TableBuildReport {
    digest: String,
    decimation: u32,
    grid: GridSummary,
    total_population: f64,
    cells: CellTalliesReport,
    cache: CacheFiles,
}

#[derive(Debug, Clone, Serialize)]
pub struct CacheFiles {
    header: String,
    payload: String,
}

impl CacheFiles {
    /// A path is published with whatever is not UTF-8 replaced, because a document a renderer parses is
    /// UTF-8 and a path is not promised to be.
    fn new(header: &Path, payload: &Path) -> Self {
        Self {
            header: header.to_string_lossy().into_owned(),
            payload: payload.to_string_lossy().into_owned(),
        }
    }
}

impl TableBuildReport {
    #[must_use]
    pub fn new(built: &BuiltTable, header: &Path, payload: &Path) -> Self {
        Self {
            digest: hexadecimal(built.digest),
            decimation: built.decimation.factor(),
            grid: GridSummary::from(built.decimation.grid()),
            total_population: built.total,
            cells: built.tallies.into(),
            cache: CacheFiles::new(header, payload),
        }
    }
}

/// The population of one rectangle of a table, with the rectangle the table resolved the request to.
///
/// `window` is absent when the request was the table's whole extent, which is not a window any pair of
/// coordinates expresses — [`Table::whole`](crate::table::Table::whole) says why.
#[derive(Debug, Clone, Serialize)]
pub struct TableQueryReport {
    digest: String,
    grid: GridSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    window: Option<WindowReport>,
    rows: RowRange,
    columns: ColRange,
    population: f64,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct WindowReport {
    north: f64,
    south: f64,
    west: f64,
    east: f64,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct RowRange {
    north: u32,
    south: u32,
}

/// The columns covered, with the full turn stated rather than left to be inferred from `west` and
/// `east`: on a grid whose columns close, one column and all of them are the same pair of indices.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ColRange {
    west: u32,
    east: u32,
    full_turn: bool,
}

impl TableQueryReport {
    #[must_use]
    pub fn new(
        digest: u64,
        grid: &Grid,
        window: Option<Window>,
        rows: RowBand,
        cols: ColSpan,
        population: f64,
    ) -> Self {
        let columns = match cols {
            ColSpan::FullTurn => ColRange {
                west: 0,
                east: grid.width() - 1,
                full_turn: true,
            },
            ColSpan::Through { west, east } => ColRange {
                west: west.get(),
                east: east.get(),
                full_turn: false,
            },
        };
        Self {
            digest: hexadecimal(digest),
            grid: GridSummary::from(grid),
            window: window.map(|window| WindowReport {
                north: window.north,
                south: window.south,
                west: window.west,
                east: window.east,
            }),
            rows: RowRange {
                north: rows.north().get(),
                south: rows.south().get(),
            },
            columns,
            population,
        }
    }
}

/// The one spelling of a digest, so the string a build publishes is the string a query accepts.
fn hexadecimal(digest: u64) -> String {
    format!("{digest:#018x}")
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this
// narrow exemption; docs/ai/code.md allows both in tests.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::geodesy::great_circle_km;
    use crate::table::Decimation;

    #[test]
    fn the_envelope_leads_with_its_schema_version() {
        // Asserted on the text rather than on a parsed value, because what needs pinning is that
        // the version is the *first* key: a consumer streaming the document reads it before it has
        // to understand anything else.
        let json = serde_json::to_string(&Envelope::new(())).unwrap();
        assert!(json.starts_with(r#"{"schema_version":1,"#), "{json}");
    }

    /// The provenance of a table over [`degree_grid`], for the two tests that want one and vary nothing
    /// about it.
    fn provenance() -> Provenance {
        Provenance::new(
            &Identity {
                digest: 0x3a5d_5e3b_082f_2fb7,
                decimation: Decimation::none(degree_grid()),
            },
            Path::new("out/table.header.json"),
            Path::new("out/table.payload.bin"),
        )
    }

    #[test]
    fn an_envelope_without_provenance_carries_no_key_for_it() {
        // The absent case as a substring rather than as a parsed value: what the skip promises is that
        // the key is not there at all, and a consumer distinguishing absent from null reads the text.
        let json = serde_json::to_string(&Envelope::new(())).unwrap();
        assert!(!json.contains("provenance"), "{json}");
    }

    #[test]
    fn provenance_is_published_before_the_result_it_produced() {
        let json = serde_json::to_string(&Envelope::with_provenance((), provenance())).unwrap();
        let at = json.find(r#""provenance":"#).expect("the key is emitted");
        let result = json.find(r#""result":"#).expect("the payload is emitted");
        assert!(at < result, "{json}");
    }

    #[test]
    fn a_published_longitude_is_reduced() {
        // Grid::centre_of returns longitudes past 180 for a window crossing the antimeridian, so
        // this conversion is the seam where that stops being the consumer's problem.
        let json = serde_json::to_string(&Coordinate::from(LatLon {
            lat: 12.5,
            lon: 190.0,
        }))
        .unwrap();
        assert_eq!(json, r#"{"lat":12.5,"lon":-170.0}"#);
    }

    // The snapshots below are the wire format itself: they fail on a renamed field, a reordered one
    // and a changed number alike, which is what a document read by two renderers and written by two
    // command surfaces needs. Each input is fixed and named for what makes it a good witness.
    #[test]
    fn the_distance_document_holds_its_shape() {
        // The quarter circumference: a value checkable against the sphere by hand, unlike a pair of
        // cities, so a snapshot accepted by mistake is visible as a wrong number rather than only as
        // a diff.
        let from = LatLon { lat: 0.0, lon: 0.0 };
        let to = LatLon {
            lat: 0.0,
            lon: 90.0,
        };
        // Rounded because sin, cos and atan2 are not bit-identical across libm implementations: the
        // full expansion of this f64 differs in its last digits between arm64 and x86_64, so pinning
        // it would make the snapshot a test of the host rather than of the document. Six decimals is
        // a millimetre, far below anything the sphere model itself is good for. The wire format still
        // carries the unrounded value.
        insta::assert_json_snapshot!(
            Envelope::new(DistanceReport::new(from, to, great_circle_km(from, to))),
            { ".result.great_circle_km" => insta::rounded_redaction(6) }
        );
    }

    /// A one-degree whole-globe grid, small enough to read and closed in longitude, so the full turn in
    /// the query document below is a case this grid actually has.
    fn degree_grid() -> Grid {
        Grid::new(
            360,
            180,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            1.0,
            -1.0,
        )
        .expect("a 1 degree whole-globe grid is valid")
    }

    #[test]
    fn the_table_build_document_holds_its_shape() {
        // Assembled rather than built, so the numbers are the ones a reader can check against the fields
        // they land in: the tallies sum to the grid's cell count, and the digest is the value a query
        // has to pass back verbatim.
        let built = BuiltTable {
            digest: 0x3a5d_5e3b_082f_2fb7,
            tallies: CellTallies {
                nodata: 40_000,
                unexpected_negative: 0,
                zero: 8_000,
                populated: 16_800,
            },
            total: 7_757_982_599.32,
            decimation: Decimation::none(degree_grid()),
        };
        insta::assert_json_snapshot!(Envelope::new(TableBuildReport::new(
            &built,
            Path::new("out/table.header.json"),
            Path::new("out/table.payload.bin"),
        )));
    }

    #[test]
    fn the_table_query_document_holds_its_shape() {
        let grid = degree_grid();
        let rows = RowBand::new(
            grid.row(0).expect("a row of the fixture"),
            grid.row(179).expect("a row of the fixture"),
        );
        // The full turn, because that is the case whose `west` and `east` a consumer cannot infer.
        insta::assert_json_snapshot!(Envelope::new(TableQueryReport::new(
            0x3a5d_5e3b_082f_2fb7,
            &grid,
            None,
            rows,
            ColSpan::FullTurn,
            7_757_982_599.32,
        )));
    }

    #[test]
    fn the_grid_document_holds_its_shape() {
        let grid = Grid::new(
            360,
            180,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            1.0,
            -1.0,
        )
        .expect("a 1 degree whole-globe grid is valid");
        insta::assert_json_snapshot!(Envelope::new(GridSummary::from(&grid)));
    }
}
