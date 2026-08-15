mod args;
mod commands;
mod failure;
mod observe;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use args::{
    CachedTableArgs, GridArgs, LedgerArgs, LogArgs, RasterSpecArgs, SearchArgs, SweepArgs,
    TableArgs, WindowArgs, parse_radius, parse_share,
};
use clap::{Parser, Subcommand};
use commands::distance::distance_json;
use commands::grid::describe_grid;
use commands::search::{most_populous, population_at, smallest_for_share, sweep};
use commands::table::{build_table, query_table};
use failure::{EXIT_FAILURE, Failure};
use observe::StderrLog;
use popcircles::geodesy::RadiusKm;
use popcircles::smallest::Share;

/// Without an `about`, clap falls back to the description of the struct `Cli` flattens, and what a user
/// read first was `LogArgs`'s reasoning, which this `about` is what closed.
#[derive(Parser, Debug)]
#[command(
    name = "popcircles",
    version,
    about = "Find the smallest circle on the globe containing a given share of world population, by \
             great-circle radius rather than by area on a projected map."
)]
struct Cli {
    #[command(flatten)]
    log: LogArgs,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug, Clone)]
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
    /// The summation table and its on-disk cache.
    Table {
        #[command(subcommand)]
        command: TableCommand,
    },
    /// The population inside a circle of a given ground radius about a coordinate.
    PopulationAt {
        #[command(flatten)]
        cached: CachedTableArgs,
        /// The circle's centre. It is snapped to the centre of the cell containing it, which the document
        /// publishes beside the coordinate asked for.
        #[arg(long, allow_negative_numbers = true)]
        lat: f64,
        #[arg(long, allow_negative_numbers = true)]
        lon: f64,
        /// The circle's ground radius: a great-circle arc on the sphere, never a distance in degrees.
        #[arg(long, allow_negative_numbers = true, value_parser = parse_radius)]
        radius_km: RadiusKm,
    },
    /// The most populous circle of a fixed ground radius, over every cell centre of the grid.
    MostPopulous {
        #[command(flatten)]
        cached: CachedTableArgs,
        /// The circle's ground radius: a great-circle arc on the sphere, never a distance in degrees.
        #[arg(long, allow_negative_numbers = true, value_parser = parse_radius)]
        radius_km: RadiusKm,
        #[command(flatten)]
        search: SearchArgs,
    },
    /// The smallest circle whose population reaches a share of the table's own total.
    SmallestForShare {
        #[command(flatten)]
        cached: CachedTableArgs,
        /// The share to reach, in whole percent. A hundred is everyone the table holds.
        #[arg(long, value_parser = parse_share)]
        share: Share,
        #[command(flatten)]
        search: SearchArgs,
        #[command(flatten)]
        ledger: LedgerArgs,
    },
    /// The smallest circle for each of a range of shares, over one ledger.
    Sweep {
        #[command(flatten)]
        cached: CachedTableArgs,
        #[command(flatten)]
        shares: SweepArgs,
        #[command(flatten)]
        search: SearchArgs,
        #[command(flatten)]
        ledger: LedgerArgs,
    },
}

#[derive(Subcommand, Debug, Clone, Copy)]
enum GridCommand {
    /// Describe a north-up grid without reading any raster.
    Describe {
        #[command(flatten)]
        grid: GridArgs,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum TableCommand {
    /// Build a summation table from a raster and publish it to the cache.
    Build {
        /// The raster to read. Its tags must agree with the grid declared below.
        #[arg(long)]
        raster: PathBuf,
        #[command(flatten)]
        grid: GridArgs,
        #[command(flatten)]
        raster_spec: RasterSpecArgs,
        #[command(flatten)]
        table: TableArgs,
    },
    /// Query a cached table for the population of a rectangle, reading it by mmap.
    Query {
        #[command(flatten)]
        cached: CachedTableArgs,
        #[command(flatten)]
        window: WindowArgs,
    },
}

fn main() -> ExitCode {
    // The first statement, before argument parsing: elapsed is measured from the process
    // started, and a clock started after `Cli::parse()` and the install is not that.
    let started = Instant::now();
    let cli = Cli::parse();
    StderrLog::install(started, cli.log.log_level);

    match run(cli.command) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(failure) => {
            eprintln!("{}", failure.message);
            ExitCode::from(failure.code)
        }
    }
}

fn run(command: Command) -> Result<String, Failure> {
    match command {
        Command::Distance {
            from_lat,
            from_lon,
            to_lat,
            to_lon,
        } => distance_json(from_lat, from_lon, to_lat, to_lon).map_err(|error| Failure {
            code: EXIT_FAILURE,
            message: format!("{error:?}"),
        }),
        Command::Grid {
            command: GridCommand::Describe { grid },
        } => describe_grid(grid),
        Command::Table {
            command:
                TableCommand::Build {
                    raster,
                    grid,
                    raster_spec,
                    table,
                },
        } => build_table(&raster, grid, raster_spec, &table),
        Command::Table {
            command: TableCommand::Query { cached, window },
        } => query_table(&cached, window.window()),
        Command::PopulationAt {
            cached,
            lat,
            lon,
            radius_km,
        } => population_at(&cached, lat, lon, radius_km),
        Command::MostPopulous {
            cached,
            radius_km,
            search,
        } => most_populous(&cached, radius_km, search),
        Command::SmallestForShare {
            cached,
            share,
            search,
            ledger,
        } => smallest_for_share(&cached, share, search, &ledger),
        Command::Sweep {
            cached,
            shares,
            search,
            ledger,
        } => sweep(&cached, shares, search, &ledger),
    }
}

// unwrap and expect are both warn at workspace level and lint:rust runs --all-targets, so tests need
// this narrow exemption; docs/ai/code.md allows both in tests. Both are load-bearing here, unlike the
// other test modules in this crate: the flag-list helper unwraps and the parser assertions expect.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use log::LevelFilter;

    use super::*;

    /// The six grid flags and the three cache ones every cached-table command takes, so a test naming a
    /// command spells out only what is its own.
    fn cached_table_flags() -> [&'static str; 14] {
        [
            "--width",
            "4",
            "--height",
            "3",
            "--origin-lat",
            "90",
            "--origin-lon",
            "-180",
            "--lon-step",
            "90",
            "--lat-step",
            "-60",
            "--digest",
            "0x1",
        ]
    }

    #[test]
    fn a_radius_that_is_not_a_length_is_refused_by_the_parser() {
        // At the parser, which is what makes it exit 2 without a cache being opened: `RadiusKm::new` holds
        // the two checks and clap reports whichever one fired.
        assert!(parse_radius("-1").is_err());
        assert!(parse_radius("nan").is_err());
        assert!(parse_radius("inf").is_err());
        assert!(parse_radius("wide").is_err());
        // Zero is a radius — the circle is its own centre cell — and so is a figure past half the globe.
        assert_eq!(parse_radius("0"), Ok(RadiusKm::new(0.0).unwrap()));
        assert_eq!(parse_radius("20016"), Ok(RadiusKm::new(20_016.0).unwrap()));

        let negative = Cli::try_parse_from(
            ["popcircles", "population-at"]
                .into_iter()
                .chain(cached_table_flags())
                .chain(["--lat", "0", "--lon", "0", "--radius-km", "-1"]),
        );
        assert!(negative.is_err());
    }

    #[test]
    fn a_search_without_a_spacing_is_a_usage_error() {
        // Required rather than defaulted, which is FU-08's distinction: a command that forwards the
        // caller's number chooses nothing, and inventing a figure here is what that entry exists to stop.
        let named = ["popcircles", "most-populous"];
        let circle = ["--radius-km", "3000"];

        let without =
            Cli::try_parse_from(named.into_iter().chain(cached_table_flags()).chain(circle));
        assert!(without.is_err());

        let with = Cli::try_parse_from(
            named
                .into_iter()
                .chain(cached_table_flags())
                .chain(circle)
                .chain(["--spacing", "8"]),
        );
        assert!(with.is_ok(), "{with:?}");

        // Zero is not a spacing, and `NonZeroU32` is what refuses it rather than a check in this crate.
        let zero = Cli::try_parse_from(
            named
                .into_iter()
                .chain(cached_table_flags())
                .chain(circle)
                .chain(["--spacing", "0"]),
        );
        assert!(zero.is_err());
    }

    #[test]
    fn the_level_is_taken_before_a_subcommand_and_after_it() {
        // What `global` on the argument buys: one declaration, accepted on either side of the subcommand
        // name, so no subcommand carries a copy of the flag.
        let before = Cli::try_parse_from([
            "popcircles",
            "--log-level",
            "debug",
            "distance",
            "0",
            "0",
            "0",
            "90",
        ])
        .expect("the flag is global");
        assert_eq!(before.log.log_level, LevelFilter::Debug);

        let after = Cli::try_parse_from([
            "popcircles",
            "distance",
            "0",
            "0",
            "0",
            "90",
            "--log-level",
            "warn",
        ])
        .expect("the flag is global");
        assert_eq!(after.log.log_level, LevelFilter::Warn);

        // The default is `info`, per box 5.
        let neither = Cli::try_parse_from(["popcircles", "distance", "0", "0", "0", "90"])
            .expect("the level defaults");
        assert_eq!(neither.log.log_level, LevelFilter::Info);
    }

    #[test]
    fn a_window_needs_all_four_bounds_or_none() {
        // The group is what makes a half-given window a usage error rather than a query over an extent
        // the caller did not mean.
        let none = Cli::try_parse_from([
            "popcircles",
            "table",
            "query",
            "--width",
            "4",
            "--height",
            "3",
            "--origin-lat",
            "90",
            "--origin-lon",
            "-180",
            "--lon-step",
            "90",
            "--lat-step",
            "-60",
            "--digest",
            "0x1",
        ]);
        assert!(none.is_ok(), "{none:?}");

        let half = Cli::try_parse_from([
            "popcircles",
            "table",
            "query",
            "--width",
            "4",
            "--height",
            "3",
            "--origin-lat",
            "90",
            "--origin-lon",
            "-180",
            "--lon-step",
            "90",
            "--lat-step",
            "-60",
            "--digest",
            "0x1",
            "--north",
            "90",
        ]);
        assert!(half.is_err());
    }
}
