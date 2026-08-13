use std::process::ExitCode;

use anyhow::Context;
use clap::{Parser, Subcommand};
use popcircles::geodesy::{LatLon, great_circle_km};
use popcircles::report::{DistanceReport, Envelope};

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
        from_lat: f64,
        from_lon: f64,
        to_lat: f64,
        to_lon: f64,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse().command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err:?}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> anyhow::Result<()> {
    match command {
        Command::Distance {
            from_lat,
            from_lon,
            to_lat,
            to_lon,
        } => {
            let from = LatLon {
                lat: from_lat,
                lon: from_lon,
            };
            let to = LatLon {
                lat: to_lat,
                lon: to_lon,
            };
            let report = DistanceReport::new(from, to, great_circle_km(from, to));
            let json = serde_json::to_string(&Envelope::new(report))
                .context("serialising the distance report")?;
            println!("{json}");
            Ok(())
        }
    }
}
