use std::process::ExitCode;

use anyhow::Context;
use clap::{Parser, Subcommand};
use popcircles::geodesy::{LatLon, great_circle_km};
use popcircles::grid::{Grid, GridError};
use popcircles::report::{DistanceReport, Envelope, GridSummary};

#[derive(Parser, Debug)]
#[command(name = "popcircles", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug, Clone, Copy)]
enum Command {
    /// Great-circle distance between two coordinates, in kilometres.
    Distance {
        #[arg(allow_negative_numbers = true)]
        from_lat: f64,
        #[arg(allow_negative_numbers = true)]
        from_lon: f64,
        #[arg(allow_negative_numbers = true)]
        to_lat: f64,
        #[arg(allow_negative_numbers = true)]
        to_lon: f64,
    },
    /// Raster grid geometry.
    Grid {
        #[command(subcommand)]
        command: GridCommand,
    },
}

#[derive(Subcommand, Debug, Clone, Copy)]
enum GridCommand {
    /// Describe a north-up grid without reading any raster.
    Describe {
        #[arg(long)]
        width: u32,
        #[arg(long)]
        height: u32,
        #[arg(long, allow_negative_numbers = true)]
        origin_lat: f64,
        #[arg(long, allow_negative_numbers = true)]
        origin_lon: f64,
        #[arg(long, allow_negative_numbers = true)]
        lon_step: f64,
        #[arg(long, allow_negative_numbers = true)]
        lat_step: f64,
    },
}

/// Bad input is one class today; `application.md`'s "missing data" and "interrupted" classes have
/// no caller yet and are not coded here ahead of one.
const EXIT_BAD_INPUT: u8 = 2;

fn main() -> ExitCode {
    run(Cli::parse().command)
}

fn run(command: Command) -> ExitCode {
    match command {
        Command::Distance {
            from_lat,
            from_lon,
            to_lat,
            to_lon,
        } => match distance_json(from_lat, from_lon, to_lat, to_lon) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("{err:?}");
                ExitCode::FAILURE
            }
        },
        Command::Grid {
            command:
                GridCommand::Describe {
                    width,
                    height,
                    origin_lat,
                    origin_lon,
                    lon_step,
                    lat_step,
                },
        } => match describe_grid(width, height, origin_lat, origin_lon, lon_step, lat_step) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(GridDescribeError::Grid(err)) => {
                eprintln!("{err}");
                ExitCode::from(exit_code_for_grid_error(&err))
            }
            Err(GridDescribeError::Serialize(err)) => {
                eprintln!("{err:?}");
                ExitCode::FAILURE
            }
        },
    }
}

fn distance_json(from_lat: f64, from_lon: f64, to_lat: f64, to_lon: f64) -> anyhow::Result<String> {
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

enum GridDescribeError {
    Grid(GridError),
    Serialize(serde_json::Error),
}

fn describe_grid(
    width: u32,
    height: u32,
    origin_lat: f64,
    origin_lon: f64,
    lon_step: f64,
    lat_step: f64,
) -> Result<String, GridDescribeError> {
    let origin = LatLon {
        lat: origin_lat,
        lon: origin_lon,
    };
    let grid =
        Grid::new(width, height, origin, lon_step, lat_step).map_err(GridDescribeError::Grid)?;
    serde_json::to_string(&Envelope::new(GridSummary::from(&grid)))
        .map_err(GridDescribeError::Serialize)
}

/// The only exit-code class that exists yet: bad input. A variant `GridError` gains later has no
/// arm here to fall into, so this crate fails to build until one is added — the property the
/// exhaustive match exists to hold.
const fn exit_code_for_grid_error(error: &GridError) -> u8 {
    match error {
        GridError::EmptyDimension { .. }
        | GridError::NonFiniteOrigin { .. }
        | GridError::OriginLatOutOfRange { .. }
        | GridError::NonFiniteStep { .. }
        | GridError::ZeroStep { .. }
        | GridError::LatStepNotSouthward { .. }
        | GridError::RunsPastSouthPole { .. }
        | GridError::RunsPastAFullTurn { .. } => EXIT_BAD_INPUT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_grid_error_is_bad_input() {
        let errors = [
            GridError::EmptyDimension {
                width: 0,
                height: 0,
            },
            GridError::NonFiniteOrigin {
                origin: LatLon {
                    lat: f64::NAN,
                    lon: 0.0,
                },
            },
            GridError::OriginLatOutOfRange { origin_lat: 91.0 },
            GridError::NonFiniteStep {
                lon_step: f64::NAN,
                lat_step: -1.0,
            },
            GridError::ZeroStep {
                lon_step: 0.0,
                lat_step: -1.0,
            },
            GridError::LatStepNotSouthward { lat_step: 1.0 },
            GridError::RunsPastSouthPole { south_edge: -91.0 },
            GridError::RunsPastAFullTurn { lon_span: 361.0 },
        ];
        for error in errors {
            assert_eq!(exit_code_for_grid_error(&error), EXIT_BAD_INPUT);
        }
    }
}
