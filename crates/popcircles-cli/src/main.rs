mod args;
mod failure;
mod observe;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use anyhow::Context;
use args::{
    CachedTableArgs, GridArgs, LedgerArgs, LogArgs, RasterSpecArgs, SearchArgs, SweepArgs,
    TableArgs, WindowArgs, parse_radius, parse_share,
};
use clap::{Parser, Subcommand};
use failure::{EXIT_FAILURE, Failure};
use observe::{StderrLog, StderrProgress};
use popcircles::bracket::Bracket;
use popcircles::circle;
use popcircles::geodesy::{LatLon, RadiusKm, great_circle_km};
use popcircles::grid::{Col, Grid, Row};
use popcircles::kernel::Kernel;
use popcircles::raster::{PixelType, RasterSpec, geotiff::GeoTiffSource};
use popcircles::report::{
    CircleReport, DistanceReport, Envelope, GridSummary, LedgerReport, MostPopulousReport,
    Provenance, SmallestDocument, SmallestReport, SweepDocument, SweepShares, TableBuildReport,
    TableQueryReport,
};
use popcircles::search;
use popcircles::smallest::cache::Ledger;
use popcircles::smallest::{self, Share};
use popcircles::table::cache::{Cache, Identity, Mapped};
use popcircles::table::{Decimation, Table, Window, build};

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

fn describe_grid(args: GridArgs) -> Result<String, Failure> {
    let grid = args.grid().map_err(|error| Failure::grid(&error))?;
    serialised(serde_json::to_string(&Envelope::new(GridSummary::from(
        &grid,
    ))))
}

fn build_table(
    raster: &Path,
    grid: GridArgs,
    raster_spec: RasterSpecArgs,
    table: &TableArgs,
) -> Result<String, Failure> {
    let grid = grid.grid().map_err(|error| Failure::grid(&error))?;
    let decimation =
        Decimation::new(grid, table.decimate).map_err(|error| Failure::table(&error))?;
    let spec = RasterSpec {
        grid,
        epsg: raster_spec.epsg,
        pixel: PixelType::Float32,
        nodata: raster_spec.nodata,
    };
    let source = GeoTiffSource::open(raster, &spec).map_err(|error| Failure::raster(&error))?;

    let cache = Cache::new(&table.cache);
    make_room_for(&table.cache)?;

    // Box 6's other half, and the reason it is not `CachedTable::open`'s record: this command opens no
    // cache. After the file was opened rather than before, so the record names a raster that is there.
    log::info!(
        "reading {} at decimation {}",
        raster.display(),
        decimation.factor()
    );

    let mut writer = cache.writer().map_err(|error| Failure::cache(&error))?;
    let mut progress = StderrProgress::new();
    let built = build(source, decimation, &mut progress, |row| {
        writer.write_row(row)
    })
    .map_err(|error| Failure::build(&error))?;
    writer
        .publish(&built)
        .map_err(|error| Failure::cache(&error))?;
    progress.finish();

    // After `finish`, so the meter's own line is closed rather than written over.
    log::info!(
        "published {} and {}",
        cache.header_path().display(),
        cache.payload_path().display()
    );

    serialised(serde_json::to_string(&Envelope::new(
        TableBuildReport::new(&built, cache.header_path(), cache.payload_path()),
    )))
}

/// A cached table, opened and mapped, with the provenance a document declares it by.
///
/// The mapping is held rather than the cells, because a [`Table`] borrows from it: keeping the two
/// together is what ties the lifetime of every query to the mapping through the compiler. Which is also
/// why [`Self::table`] is a method and not something the resolver returns — a value cannot hold a borrow
/// of its own field.
#[derive(Debug)]
struct CachedTable {
    grid: Grid,
    identity: Identity,
    mapped: Mapped,
    header: PathBuf,
    payload: PathBuf,
}

impl CachedTable {
    /// Resolves the flags to a mapped table: the declared grid, the decimation, the identity wanted, and
    /// the cache opened against it.
    ///
    /// The one place a command reads a cache, so a command asks for a table rather than assembling the
    /// path, the identity and the provenance of one for itself.
    fn open(args: &CachedTableArgs) -> Result<Self, Failure> {
        // Box 7's other half of "table build or load". The three `?`s below are exactly why the closing
        // record is `Drop`'s: a cache that is absent still says how long finding that out took.
        let _bracket = Bracket::open(module_path!(), "table load");

        let source = args.grid.grid().map_err(|error| Failure::grid(&error))?;
        let decimation =
            Decimation::new(source, args.table.decimate).map_err(|error| Failure::table(&error))?;
        let identity = Identity {
            digest: args.digest,
            decimation,
        };

        let cache = Cache::new(&args.table.cache);
        let mapped = cache
            .open(&identity)
            .map_err(|error| Failure::cache(&error))?;

        // Box 6's resolved input, here because this is already the one place a cache is opened. It names
        // what a reader would otherwise reconstruct from four flags: which table, from where, and at what
        // shape after the fold.
        let grid = decimation.grid();
        log::info!(
            "table {:#018x} opened from {}: {} x {} cells, decimated by {}",
            identity.digest,
            args.table.cache.display(),
            grid.width(),
            grid.height(),
            decimation.factor()
        );

        Ok(Self {
            grid: *grid,
            identity,
            mapped,
            header: cache.header_path().to_path_buf(),
            payload: cache.payload_path().to_path_buf(),
        })
    }

    /// The table over the mapping, and the only place in this crate one is constructed.
    fn table(&self) -> Result<Table<'_>, Failure> {
        let cells = self
            .mapped
            .cells()
            .map_err(|error| Failure::cache(&error))?;
        Table::new(self.grid, cells).map_err(|error| Failure::table(&error))
    }

    fn provenance(&self) -> Provenance {
        Provenance::new(&self.identity, &self.header, &self.payload)
    }
}

/// The cell holding `at`, or bad input naming the extent it is not on.
///
/// A function of its own so both arms are testable without a table: what makes a coordinate bad input is
/// the grid, and opening a cache to find that out would put a fixture in the way of the check.
fn centre_cell(grid: &Grid, at: LatLon) -> Result<(Row, Col), Failure> {
    grid.cell_containing(at).ok_or_else(|| {
        Failure::bad_input(format!(
            "(lat {}, lon {}) is not on a {} x {} grid whose origin is (lat {}, lon {}); a coordinate \
             on the grid's outer southern or eastern boundary lies in no cell",
            at.lat,
            at.lon,
            grid.width(),
            grid.height(),
            grid.origin().lat,
            grid.origin().lon
        ))
    })
}

fn population_at(
    cached: &CachedTableArgs,
    lat: f64,
    lon: f64,
    radius: RadiusKm,
) -> Result<String, Failure> {
    let cached = CachedTable::open(cached)?;
    let view = cached.table()?;
    let requested = LatLon { lat, lon };
    let cell = centre_cell(&cached.grid, requested)?;

    let kernel = Kernel::new(cached.grid, cell.0, radius).map_err(Failure::kernel)?;
    let population = circle::population(&view, &kernel, cell.1);
    let (rows, cols) = view.whole();
    let total = view.population(rows, cols);

    let centre = cached.grid.centre_of(cell.0, cell.1);
    log::info!(
        "a {} km circle centred (lat {:.4}, lon {:.4}) holds {population} of {total}",
        radius.km(),
        centre.lat,
        centre.lon
    );

    serialised(serde_json::to_string(&Envelope::with_provenance(
        CircleReport::new(requested, cell, &cached.grid, radius, population, total),
        cached.provenance(),
    )))
}

fn most_populous(
    cached: &CachedTableArgs,
    radius: RadiusKm,
    search: SearchArgs,
) -> Result<String, Failure> {
    let cached = CachedTable::open(cached)?;
    let view = cached.table()?;
    let (rows, cols) = view.whole();
    let total = view.population(rows, cols);

    let mut progress = StderrProgress::new();
    let found = search::most_populous(&view, radius, search.spacing, &mut progress)
        .map_err(Failure::search)?;
    progress.finish();

    let centre = cached.grid.centre_of(found.centre.row, found.centre.col);
    log::info!(
        "the most populous {} km circle is centred (lat {:.4}, lon {:.4}) and holds {} of {total}",
        radius.km(),
        centre.lat,
        centre.lon,
        found.centre.population
    );

    serialised(serde_json::to_string(&Envelope::with_provenance(
        MostPopulousReport::new(&found, &cached.grid, total),
        cached.provenance(),
    )))
}

/// The ledger at `path` for the table `wanted` names, with room made for it.
///
/// The one place this crate opens one, so a sweep cannot open a ledger per share: what a ledger records is
/// the maximum at a radius, a property of the table alone, so a twenty-five percent share reuses every
/// radius a fifty percent share paid for.
fn open_ledger(path: &Path, wanted: &Identity) -> Result<Ledger, Failure> {
    make_room_for(path)?;
    Ledger::open_or_empty(path, wanted).map_err(|error| Failure::ledger(&error))
}

/// The shares a sweep walks, ascending, each converted by [`Share::from_percent`].
///
/// A function of its own so the count and every rejection are testable without a table. The walk is over
/// integers, and the two grounds below are relations between flags rather than properties of one, which is
/// why they are not a value parser's to refuse.
fn shares(from: u32, to: u32, step: u32) -> Result<Vec<Share>, Failure> {
    if step == 0 {
        return Err(Failure::bad_input(
            "a sweep's step must be at least one percent; a step of zero never reaches its end",
        ));
    }
    if from > to {
        return Err(Failure::bad_input(format!(
            "a sweep runs from the smaller share to the larger; {from}% is above {to}%"
        )));
    }

    let mut walk = Vec::new();
    let mut percent = from;
    loop {
        walk.push(
            Share::from_percent(percent).map_err(|error| Failure::bad_input(error.to_string()))?,
        );
        // Saturating, so a step near `u32::MAX` ends the walk rather than wrapping back under `to`.
        let next = percent.saturating_add(step);
        if next > to {
            return Ok(walk);
        }
        percent = next;
    }
}

fn sweep(
    cached: &CachedTableArgs,
    range: SweepArgs,
    search: SearchArgs,
    ledger: &LedgerArgs,
) -> Result<String, Failure> {
    let cached = CachedTable::open(cached)?;
    let view = cached.table()?;
    let walk = shares(range.from, range.to, range.step)?;
    let mut ledger = open_ledger(&ledger.ledger, &cached.identity)?;

    let mut progress = StderrProgress::new();
    let mut records = Vec::with_capacity(walk.len());
    for share in walk {
        let found = smallest::smallest(&view, share, search.spacing, &mut ledger, &mut progress)
            .map_err(|error| Failure::smallest(&error))?;
        // One per settled share rather than one at the end: a sweep's answer is the whole sequence, and a
        // reader watching a long one wants each share as it lands.
        log::info!(
            "{:.0}% of the table is reached at {} km",
            share.get() * 100.0,
            found.radius_km
        );
        records.push(SmallestReport::new(&found, &cached.grid));
    }
    progress.finish();

    serialised(serde_json::to_string(&Envelope::with_provenance(
        SweepDocument::new(
            LedgerReport::new(ledger.path(), ledger.len()),
            SweepShares::new(range.from, range.to, range.step),
            records,
        ),
        cached.provenance(),
    )))
}

fn smallest_for_share(
    cached: &CachedTableArgs,
    share: Share,
    search: SearchArgs,
    ledger: &LedgerArgs,
) -> Result<String, Failure> {
    let cached = CachedTable::open(cached)?;
    let view = cached.table()?;

    // Against the same identity the table was opened with, so a ledger of some other table is refused
    // rather than resumed from — which is what makes an opt-out unnecessary.
    let mut ledger = open_ledger(&ledger.ledger, &cached.identity)?;

    let mut progress = StderrProgress::new();
    let found = smallest::smallest(&view, share, search.spacing, &mut ledger, &mut progress)
        .map_err(|error| Failure::smallest(&error))?;
    progress.finish();

    log::info!(
        "{:.0}% of the table is reached at {} km",
        share.get() * 100.0,
        found.radius_km
    );

    serialised(serde_json::to_string(&Envelope::with_provenance(
        SmallestDocument::new(
            LedgerReport::new(ledger.path(), ledger.len()),
            SmallestReport::new(&found, &cached.grid),
        ),
        cached.provenance(),
    )))
}

fn query_table(cached: &CachedTableArgs, window: Option<Window>) -> Result<String, Failure> {
    let cached = CachedTable::open(cached)?;
    let view = cached.table()?;
    let grid = cached.grid;

    let (rows, cols) = match window {
        Some(window) => view.covering(window).ok_or_else(|| {
            Failure::bad_input(format!(
                "the window is not on a {} x {} grid whose origin is (lat {}, lon {}); a coordinate \
                 on the grid's outer southern or eastern boundary lies in no cell, and the whole \
                 extent is what the query does with no window at all",
                grid.width(),
                grid.height(),
                grid.origin().lat,
                grid.origin().lon
            ))
        })?,
        None => view.whole(),
    };
    let population = view.population(rows, cols);

    // No provenance block, and that is not an omission: this document's own payload carries the digest
    // and the grid, because the table is what the command is *about*. `report`'s module documentation
    // owns that distinction.
    serialised(serde_json::to_string(&Envelope::new(
        TableQueryReport::new(
            cached.identity.digest,
            &grid,
            window,
            rows,
            cols,
            population,
        ),
    )))
}

/// Makes the directory a file this crate is about to write will live in.
///
/// Resolving where a generated file goes, and making room for it, is the shell's work — the library is
/// handed a path and never asked where one should be. Both the cache and the ledger want it, which is why
/// it is a function rather than a step inside either.
fn make_room_for(file: &Path) -> Result<(), Failure> {
    let Some(parent) = file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };
    fs::create_dir_all(parent).map_err(|error| Failure {
        code: EXIT_FAILURE,
        message: format!(
            "the directory {} could not be made: {error}",
            parent.display()
        ),
    })
}

fn serialised(json: serde_json::Result<String>) -> Result<String, Failure> {
    json.map_err(|error| Failure::new(EXIT_FAILURE, &error))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use log::LevelFilter;

    use crate::failure::EXIT_BAD_INPUT;

    use super::*;

    /// A four-by-three whole-globe grid, which is what the parsing tests below declare and what
    /// `centre_cell` is checked against: the smallest shape whose columns close.
    fn coarse_grid() -> Grid {
        Grid::new(
            4,
            3,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            90.0,
            -60.0,
        )
        .expect("a 4 x 3 whole-globe grid is valid")
    }

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
    fn a_coordinate_off_the_grid_is_bad_input_naming_the_extent() {
        let grid = coarse_grid();
        let inside = centre_cell(
            &grid,
            LatLon {
                lat: 45.0,
                lon: -90.0,
            },
        )
        .expect("the coordinate is on the grid");
        assert_eq!((inside.0.get(), inside.1.get()), (0, 1));

        // The outer southern boundary lies in no cell, which is `Grid::cell_containing`'s rule rather than
        // this crate's, and the message has to say enough for a caller to see which extent it missed.
        let outside = centre_cell(
            &grid,
            LatLon {
                lat: -90.0,
                lon: 0.0,
            },
        )
        .expect_err("the south pole is on no row of this grid");
        assert_eq!(outside.code, EXIT_BAD_INPUT);
        assert!(outside.message.contains("4 x 3"), "{}", outside.message);
        assert!(outside.message.contains("lat 90"), "{}", outside.message);
    }

    #[test]
    fn a_sweep_walks_whole_percent_and_ends_on_its_last_share() {
        let walk = shares(10, 90, 10).expect("a sweep from a tenth to nine tenths is a range");
        assert_eq!(walk.len(), 9);
        assert_eq!(walk.first().map(|share| share.get()), Some(0.1));
        assert_eq!(walk.last().map(|share| share.get()), Some(0.9));
        // Ascending and exact, which is what the records inherit.
        let values: Vec<f64> = walk.iter().map(|share| share.get()).collect();
        assert_eq!(values, vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9]);

        // A share the step would carry past the end is not answered, which is what a stepped range means.
        let short =
            shares(10, 95, 10).expect("a range whose step overshoots its end is still a range");
        assert_eq!(short.len(), 9);
        assert_eq!(short.last().map(|share| share.get()), Some(0.9));

        // One share is a sweep of one, and everyone is a share.
        let whole = shares(100, 100, 10).expect("a hundred percent is a share");
        assert_eq!(whole.len(), 1);
        assert_eq!(whole.first().map(|share| share.get()), Some(1.0));
    }

    #[test]
    fn a_sweep_with_no_step_is_refused() {
        // Refused rather than looping: a step of zero would settle the first share for ever.
        let none = shares(10, 90, 0).expect_err("zero is not a step");
        assert_eq!(none.code, EXIT_BAD_INPUT);
        assert!(none.message.contains("step"), "{}", none.message);
    }

    #[test]
    fn a_sweep_from_no_share_is_refused() {
        // `Share::from_percent` refuses it, and the reason is the domain's: a circle holding nobody is
        // satisfied by every radius there is.
        let empty = shares(0, 90, 10).expect_err("zero percent is not a share");
        assert_eq!(empty.code, EXIT_BAD_INPUT);
        // Past a hundred too, wherever the walk reaches it, rather than being silently truncated.
        assert!(shares(90, 150, 10).is_err());
    }

    #[test]
    fn a_sweep_that_runs_backwards_is_refused_rather_than_empty() {
        // The failure this one exists to prevent is the quiet one: a descending range yielding nothing at
        // all, and a document of zero records reading as a table with nobody in it.
        let backwards = shares(60, 40, 10).expect_err("a sweep does not run backwards");
        assert_eq!(backwards.code, EXIT_BAD_INPUT);
        assert!(backwards.message.contains("60%"), "{}", backwards.message);
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
