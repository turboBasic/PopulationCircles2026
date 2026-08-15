// The one command that reads no table and needs no cache: two coordinates and the arc between them.

use anyhow::Context;
use popcircles::geodesy::{LatLon, great_circle_km};
use popcircles::report::{DistanceReport, Envelope};

pub(crate) fn distance_json(
    from_lat: f64,
    from_lon: f64,
    to_lat: f64,
    to_lon: f64,
) -> anyhow::Result<String> {
    let from = LatLon {
        lat: from_lat,
        lon: from_lon,
    };
    let to = LatLon {
        lat: to_lat,
        lon: to_lon,
    };
    let report = DistanceReport::new(from, to, great_circle_km(from, to));
    serde_json::to_string(&Envelope::new(report)).context("serialising the distance report")
}
