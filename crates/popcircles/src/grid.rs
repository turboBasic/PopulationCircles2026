use std::error::Error;
use std::fmt;

use crate::geodesy::LatLon;

// A step arrives as a decimal from a geotransform and need not round-trip to the rational it
// means, so height * lat_step can land a few ulps past -90 on a grid that in fact ends there. The
// size is what keeps this from swallowing a real overrun: one row too many costs a whole cell,
// 1/120° at the finest grid here, eight orders of magnitude above this.
const POLE_TOLERANCE_DEG: f64 = 1e-9;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridError {
    EmptyDimension { width: u32, height: u32 },
    NonFiniteOrigin { origin: LatLon },
    OriginLatOutOfRange { origin_lat: f64 },
    NonFiniteStep { lon_step: f64, lat_step: f64 },
    ZeroStep { lon_step: f64, lat_step: f64 },
    LatStepNotSouthward { lat_step: f64 },
    RunsPastSouthPole { south_edge: f64 },
}

impl fmt::Display for GridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::EmptyDimension { width, height } => {
                write!(f, "grid has an empty dimension: {width} x {height}")
            }
            Self::NonFiniteOrigin { origin } => {
                write!(
                    f,
                    "grid origin is not finite: ({}, {})",
                    origin.lat, origin.lon
                )
            }
            Self::OriginLatOutOfRange { origin_lat } => {
                write!(f, "grid origin latitude {origin_lat} is outside [-90, 90]")
            }
            Self::NonFiniteStep { lon_step, lat_step } => {
                write!(f, "grid step is not finite: ({lon_step}, {lat_step})")
            }
            Self::ZeroStep { lon_step, lat_step } => {
                write!(f, "grid step is zero: ({lon_step}, {lat_step})")
            }
            Self::LatStepNotSouthward { lat_step } => write!(
                f,
                "grid lat_step {lat_step} is not negative; rows run north to south"
            ),
            Self::RunsPastSouthPole { south_edge } => {
                write!(f, "grid rows reach {south_edge}, past the south pole")
            }
        }
    }
}

impl Error for GridError {}

/// Rows run north to south, so `lat_step` is negative — the GDAL north-up convention, kept so a
/// geotransform read from a raster needs no sign flip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    width: u32,
    height: u32,
    origin: LatLon,
    lon_step: f64,
    lat_step: f64,
}

impl Grid {
    /// # Errors
    /// [`GridError`], whose variants name the cases, when the arguments cannot describe a north-up
    /// grid that stays on the globe.
    pub fn new(
        width: u32,
        height: u32,
        origin: LatLon,
        lon_step: f64,
        lat_step: f64,
    ) -> Result<Self, GridError> {
        if width == 0 || height == 0 {
            return Err(GridError::EmptyDimension { width, height });
        }
        if !origin.lat.is_finite() || !origin.lon.is_finite() {
            return Err(GridError::NonFiniteOrigin { origin });
        }
        if !(-90.0..=90.0).contains(&origin.lat) {
            return Err(GridError::OriginLatOutOfRange {
                origin_lat: origin.lat,
            });
        }
        if !lon_step.is_finite() || !lat_step.is_finite() {
            return Err(GridError::NonFiniteStep { lon_step, lat_step });
        }
        if lon_step == 0.0 || lat_step == 0.0 {
            return Err(GridError::ZeroStep { lon_step, lat_step });
        }
        if lat_step > 0.0 {
            return Err(GridError::LatStepNotSouthward { lat_step });
        }

        // u32 -> f64 is exact: f64 represents every integer below 2^53.
        let south_edge = origin.lat + f64::from(height) * lat_step;
        if south_edge < -90.0 - POLE_TOLERANCE_DEG {
            return Err(GridError::RunsPastSouthPole { south_edge });
        }

        Ok(Self {
            width,
            height,
            origin,
            lon_step,
            lat_step,
        })
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub const fn origin(&self) -> LatLon {
        self.origin
    }

    #[must_use]
    pub const fn lon_step(&self) -> f64 {
        self.lon_step
    }

    #[must_use]
    pub const fn lat_step(&self) -> f64 {
        self.lat_step
    }
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this
// narrow exemption; docs/ai/code.md allows both in tests. float_cmp is here because these
// assertions pin that the constructor stored its arguments verbatim — bit-exact equality is the
// property, not an approximation of one.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;

    const GPW_STEP: f64 = 1.0 / 120.0;

    fn gpw() -> Grid {
        Grid::new(
            43200,
            21600,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            GPW_STEP,
            -GPW_STEP,
        )
        .expect("the GPW registry row is a valid grid")
    }

    #[test]
    fn gpw_registry_row_constructs() {
        let grid = gpw();
        assert_eq!(grid.width(), 43200);
        assert_eq!(grid.height(), 21600);
        assert_eq!(grid.origin().lat, 90.0);
        assert_eq!(grid.origin().lon, -180.0);
        assert_eq!(grid.lat_step(), -GPW_STEP);
    }

    fn err(width: u32, height: u32, origin_lat: f64, lon_step: f64, lat_step: f64) -> GridError {
        Grid::new(
            width,
            height,
            LatLon {
                lat: origin_lat,
                lon: -180.0,
            },
            lon_step,
            lat_step,
        )
        .expect_err("expected this grid to be rejected")
    }

    #[test]
    fn rejects_empty_dimension() {
        assert_eq!(
            err(0, 21600, 90.0, GPW_STEP, -GPW_STEP),
            GridError::EmptyDimension {
                width: 0,
                height: 21600
            }
        );
        assert_eq!(
            err(43200, 0, 90.0, GPW_STEP, -GPW_STEP),
            GridError::EmptyDimension {
                width: 43200,
                height: 0
            }
        );
    }

    #[test]
    fn rejects_non_finite_origin() {
        assert!(matches!(
            err(43200, 21600, f64::NAN, GPW_STEP, -GPW_STEP),
            GridError::NonFiniteOrigin { .. }
        ));
        assert!(matches!(
            Grid::new(
                43200,
                21600,
                LatLon {
                    lat: 90.0,
                    lon: f64::INFINITY
                },
                GPW_STEP,
                -GPW_STEP
            ),
            Err(GridError::NonFiniteOrigin { .. })
        ));
    }

    #[test]
    fn rejects_origin_lat_out_of_range() {
        assert_eq!(
            err(43200, 21600, 90.5, GPW_STEP, -GPW_STEP),
            GridError::OriginLatOutOfRange { origin_lat: 90.5 }
        );
        assert_eq!(
            err(43200, 21600, -90.5, GPW_STEP, -GPW_STEP),
            GridError::OriginLatOutOfRange { origin_lat: -90.5 }
        );
    }

    #[test]
    fn rejects_non_finite_step() {
        assert!(matches!(
            err(43200, 21600, 90.0, f64::NAN, -GPW_STEP),
            GridError::NonFiniteStep { .. }
        ));
        assert!(matches!(
            err(43200, 21600, 90.0, GPW_STEP, f64::NEG_INFINITY),
            GridError::NonFiniteStep { .. }
        ));
    }

    #[test]
    fn rejects_zero_step() {
        assert!(matches!(
            err(43200, 21600, 90.0, 0.0, -GPW_STEP),
            GridError::ZeroStep { .. }
        ));
        assert!(matches!(
            err(43200, 21600, 90.0, GPW_STEP, 0.0),
            GridError::ZeroStep { .. }
        ));
    }

    #[test]
    fn rejects_northward_lat_step() {
        assert_eq!(
            err(43200, 21600, -90.0, GPW_STEP, GPW_STEP),
            GridError::LatStepNotSouthward { lat_step: GPW_STEP }
        );
    }

    #[test]
    fn rejects_rows_past_the_south_pole() {
        // One row too many at the GPW step overruns by a whole cell, well past the rounding the
        // tolerance admits.
        assert!(matches!(
            err(43200, 21601, 90.0, GPW_STEP, -GPW_STEP),
            GridError::RunsPastSouthPole { .. }
        ));
    }

    #[test]
    fn a_step_a_hair_too_large_still_reaches_the_pole() {
        // What POLE_TOLERANCE_DEG is for: this grid ends at -90 in intent but computes a south edge
        // just past it. Rejecting it would fail a real raster on a rounding artefact.
        let step = f64::from_bits((GPW_STEP).to_bits() + 1);
        assert!(90.0 + f64::from(21600u32) * -step < -90.0);
        Grid::new(
            43200,
            21600,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            step,
            -step,
        )
        .expect("a step one ulp large is rounding, not an overrun");
    }

    #[test]
    fn a_partial_grid_north_of_the_pole_constructs() {
        // A country mask is a window on the globe, not the whole globe: rows stopping short of -90
        // are the normal case, not an edge case.
        Grid::new(
            1200,
            600,
            LatLon {
                lat: 60.0,
                lon: -10.0,
            },
            GPW_STEP,
            -GPW_STEP,
        )
        .expect("a window grid inside the globe is valid");
    }
}
