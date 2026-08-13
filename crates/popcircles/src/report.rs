// The published shape of a result, and the only place in this crate a serde derive appears. ADR 0001
// decision 3: the domain types change when the search changes, so what is serialised is a separate
// representation with its own version, and a field here is a promise to two renderers and two
// command surfaces.
use serde::Serialize;

use crate::geodesy::{LatLon, wrap_lon};
use crate::grid::Grid;

/// Bumped when a change to a document below is not additive — a renamed or removed field, or one
/// whose meaning moved. A new field does not bump it.
pub const SCHEMA_VERSION: u32 = 1;

/// Every document the program writes.
///
/// `schema_version` is declared first because serde emits struct fields in declaration order, so a
/// consumer reads the version before anything it might not understand.
#[derive(Debug, Clone, Serialize)]
pub struct Envelope<T> {
    schema_version: u32,
    tool: &'static str,
    tool_version: &'static str,
    result: T,
}

impl<T> Envelope<T> {
    #[must_use]
    pub const fn new(result: T) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            // This crate, not whichever binary is writing: the format is the library's, and stamping
            // the caller's name would make one document's producer unidentifiable from another's.
            tool: env!("CARGO_PKG_NAME"),
            tool_version: env!("CARGO_PKG_VERSION"),
            result,
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

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this
// narrow exemption; docs/ai/code.md allows both in tests.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::geodesy::great_circle_km;

    #[test]
    fn the_envelope_leads_with_its_schema_version() {
        // Asserted on the text rather than on a parsed value, because what needs pinning is that
        // the version is the *first* key: a consumer streaming the document reads it before it has
        // to understand anything else.
        let json = serde_json::to_string(&Envelope::new(())).unwrap();
        assert!(json.starts_with(r#"{"schema_version":1,"#), "{json}");
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
        insta::assert_json_snapshot!(Envelope::new(DistanceReport::new(
            from,
            to,
            great_circle_km(from, to)
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
