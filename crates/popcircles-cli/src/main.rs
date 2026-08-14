use std::fs;
use std::io::{IsTerminal, Write};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use popcircles::circle;
use popcircles::geodesy::{LatLon, RadiusKm, great_circle_km};
use popcircles::grid::{Col, Grid, GridError, Row};
use popcircles::kernel::{Kernel, KernelError};
use popcircles::progress::Progress;
use popcircles::raster::{PixelType, RasterError, RasterSpec, geotiff::GeoTiffSource};
use popcircles::report::{
    CircleReport, DistanceReport, Envelope, GridSummary, LedgerReport, MostPopulousReport,
    Provenance, SmallestDocument, SmallestReport, SweepDocument, SweepShares, TableBuildReport,
    TableQueryReport,
};
use popcircles::search::{self, SearchError};
use popcircles::smallest::cache::{Ledger, LedgerError};
use popcircles::smallest::{self, Share, SmallestError};
use popcircles::table::cache::{Cache, CacheError, Identity, Mapped};
use popcircles::table::{BuildError, Decimation, Table, TableError, Window, build};

#[derive(Parser, Debug)]
#[command(name = "popcircles", version)]
struct Cli {
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

/// The range of shares a sweep walks, in whole percent.
///
/// Integers rather than fractions, and the walk is over them: a step of a tenth accumulated in f64 reaches
/// `0.30000000000000004` by its third share and publishes it. Dividing each integer by a hundred instead
/// gives the f64 a caller typing the fraction would have got, with no accumulation anywhere.
#[derive(Args, Debug, Clone, Copy)]
struct SweepArgs {
    /// The first share to answer, in whole percent.
    #[arg(long)]
    from: u32,
    /// The last share to answer, in whole percent. A share the step would carry past it is not answered.
    #[arg(long)]
    to: u32,
    /// How much to raise the share by between records, in whole percent.
    #[arg(long)]
    step: u32,
}

/// Where the radii a run settles are kept, so an interrupted run resumes instead of paying for them
/// twice.
///
/// On by default and with no way to turn it off: a ledger describing another table is refused rather than
/// resumed from, so there is nothing an opt-out would protect against.
#[derive(Args, Debug, Clone)]
struct LedgerArgs {
    /// The JSON document every probe's maximum is recorded in. Under `out/` beside the cache, which is
    /// gitignored.
    #[arg(long, default_value = "out/radii.json")]
    ledger: PathBuf,
}

/// What the branch and bound needs beyond the circle it is looking for.
///
/// Required and with no default, deliberately. The search answers the same thing at every spacing —
/// refinement runs to single cells — so what this changes is only how long it takes, and the useful range
/// is a measured property of the raster and the radius that nothing here has measured. A default would
/// make this crate the author of a figure it took from nowhere.
#[derive(Args, Debug, Clone, Copy)]
struct SearchArgs {
    /// The side, in cells, of the blocks the first level is tiled into. It changes how long the search
    /// takes and not what it answers: tiles as wide as one cell are a brute force over every centre, and
    /// tiles wide enough that a block's bound covers the globe prune nothing.
    #[arg(long)]
    spacing: NonZeroU32,
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

/// A table that has already been built, named the way opening one needs: the grid it was declared over,
/// where the cache sits, and the digest that says which table is wanted.
///
/// Flattened into every command that reads a cached table, so those flags have one spelling and one help
/// string rather than a copy per command.
#[derive(Args, Debug, Clone)]
struct CachedTableArgs {
    #[command(flatten)]
    grid: GridArgs,
    #[command(flatten)]
    table: TableArgs,
    /// The digest a build reported, which is what names the table wanted.
    #[arg(long, value_parser = parse_digest)]
    digest: u64,
}

/// The grid a raster is declared to be. The declared grid wins over the file's own tags, which is why
/// every command that reads a raster or a table over one asks for the same six numbers.
#[derive(Args, Debug, Clone, Copy)]
struct GridArgs {
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
}

impl GridArgs {
    fn grid(self) -> Result<Grid, GridError> {
        Grid::new(
            self.width,
            self.height,
            LatLon {
                lat: self.origin_lat,
                lon: self.origin_lon,
            },
            self.lon_step,
            self.lat_step,
        )
    }
}

/// What the file must say about itself beyond its grid. No defaults: `data/README.md` owns each
/// dataset's sentinel and CRS, and a copy of them here would be a second owner drifting from the first.
#[derive(Args, Debug, Clone, Copy)]
struct RasterSpecArgs {
    /// The nodata sentinel the file declares, compared bit for bit.
    #[arg(long, allow_negative_numbers = true)]
    nodata: f32,
    #[arg(long)]
    epsg: u16,
}

#[derive(Args, Debug, Clone)]
struct TableArgs {
    /// Both cache files are this path plus a suffix. Under `out/` by default, which is gitignored —
    /// a generated table is never committed.
    #[arg(long, default_value = "out/table")]
    cache: PathBuf,
    /// Fold every k by k block of cells into one table cell. Must divide both grid dimensions.
    #[arg(long, default_value_t = 1)]
    decimate: u32,
}

/// The rectangle a query covers. All four or none: without them the query is the table's whole extent,
/// which is not a window a pair of coordinates can express — a full turn and one column reduce alike.
#[derive(Args, Debug, Clone, Copy)]
#[group(multiple = true, requires_all = ["north", "south", "west", "east"])]
struct WindowArgs {
    #[arg(long, allow_negative_numbers = true)]
    north: Option<f64>,
    #[arg(long, allow_negative_numbers = true)]
    south: Option<f64>,
    #[arg(long, allow_negative_numbers = true)]
    west: Option<f64>,
    #[arg(long, allow_negative_numbers = true)]
    east: Option<f64>,
}

impl WindowArgs {
    fn window(self) -> Option<Window> {
        Some(Window {
            north: self.north?,
            south: self.south?,
            west: self.west?,
            east: self.east?,
        })
    }
}

/// Bad input, and data that is not there; `application.md`'s "interrupted" class has no caller yet and
/// is not coded here ahead of one.
const EXIT_BAD_INPUT: u8 = 2;
const EXIT_MISSING_DATA: u8 = 3;

fn main() -> ExitCode {
    match run(Cli::parse().command) {
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

/// A failure and the exit-code class it falls in, so every command reports through one path and no
/// arm of `run` decides for itself what stdout and stderr are for.
#[derive(Debug)]
struct Failure {
    code: u8,
    message: String,
}

impl Failure {
    fn grid(error: &GridError) -> Self {
        Self::new(exit_code_for_grid_error(error), error)
    }

    fn table(error: &TableError) -> Self {
        Self::new(exit_code_for_table_error(error), error)
    }

    fn raster(error: &RasterError) -> Self {
        Self::new(exit_code_for_raster_error(error), error)
    }

    fn cache(error: &CacheError) -> Self {
        Self::new(exit_code_for_cache_error(error), error)
    }

    fn build(error: &BuildError<CacheError>) -> Self {
        Self::new(exit_code_for_build_error(error), error)
    }

    /// By value rather than by reference, unlike its neighbours: a `KernelError` is one f64 wide, and
    /// clippy's copy threshold is what settles that rather than a preference.
    fn kernel(error: KernelError) -> Self {
        Self::new(exit_code_for_kernel_error(error), &error)
    }

    /// By value for [`Self::kernel`]'s reason: a `SearchError` wraps one and is no wider.
    fn search(error: SearchError) -> Self {
        Self::new(exit_code_for_search_error(error), &error)
    }

    /// Bad input this crate has diagnosed itself, where there is no library error to carry the sentence:
    /// a coordinate off the grid, a window off it, a sweep that runs backwards.
    fn bad_input(message: impl Into<String>) -> Self {
        Self {
            code: EXIT_BAD_INPUT,
            message: message.into(),
        }
    }

    fn ledger(error: &LedgerError) -> Self {
        Self::new(exit_code_for_ledger_error(error), error)
    }

    fn smallest(error: &SmallestError<LedgerError>) -> Self {
        Self::new(exit_code_for_smallest_error(error), error)
    }

    /// The whole source chain, because a `thiserror` message is one link and the sentence naming the
    /// file or the syscall is usually the one beneath it.
    fn new(code: u8, error: &dyn std::error::Error) -> Self {
        let mut message = error.to_string();
        let mut source = error.source();
        while let Some(error) = source {
            message.push_str(": ");
            message.push_str(&error.to_string());
            source = error.source();
        }
        Self { code, message }
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
        Ok(Self {
            grid: *decimation.grid(),
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

/// Accepts what a build printed, `0x` and all, because the digest a query needs is copied from one
/// document into one flag.
fn parse_digest(value: &str) -> Result<u64, String> {
    let digits = value.strip_prefix("0x").unwrap_or(value);
    u64::from_str_radix(digits, 16)
        .map_err(|error| format!("`{value}` is not a 64-bit hexadecimal digest: {error}"))
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

/// A radius through [`RadiusKm::new`], so a negative or non-finite one is a usage error the parser
/// reports.
///
/// Exit 2 is what clap gives a usage error, which is already `EXIT_BAD_INPUT`, so `RadiusError` needs no
/// arm of its own in the classifiers below.
fn parse_radius(value: &str) -> Result<RadiusKm, String> {
    let km: f64 = value
        .parse()
        .map_err(|error| format!("`{value}` is not a number of kilometres: {error}"))?;
    RadiusKm::new(km).map_err(|error| error.to_string())
}

fn serialised(json: serde_json::Result<String>) -> Result<String, Failure> {
    json.map_err(|error| Failure::new(EXIT_FAILURE, &error))
}

/// Progress on stderr, which is ADR 0001 decision 4's other half: the library reports through a sink,
/// and choosing the stream is this crate's business.
///
/// One line, redrawn per whole percent, and silent when stderr is not a terminal — a redraw in a log
/// file is a hundred lines of carriage returns.
#[derive(Debug)]
struct StderrProgress {
    interactive: bool,
    percent: Option<u64>,
}

impl StderrProgress {
    fn new() -> Self {
        Self {
            interactive: std::io::stderr().is_terminal(),
            percent: None,
        }
    }

    fn finish(&mut self) {
        if self.interactive && self.percent.is_some() {
            eprintln!();
        }
    }
}

impl Progress for StderrProgress {
    fn advance(&mut self, done: u64, total: u64) {
        if !self.interactive || total == 0 {
            return;
        }
        let percent = done * 100 / total;
        if self.percent == Some(percent) {
            return;
        }
        self.percent = Some(percent);

        // A meter that cannot be drawn is not worth failing a build over: the document on stdout is
        // the result, and this is only how far it has got.
        let mut stderr = std::io::stderr();
        let _ = write!(stderr, "\r{percent:>3}% of {total} rows");
        let _ = stderr.flush();
    }
}

const EXIT_FAILURE: u8 = 1;

/// Bad input is the only class a `GridError` has. A variant it gains later has no arm here to fall
/// into, so this crate fails to build until one is added — the property every exhaustive match below
/// exists to hold.
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

/// The factor is a flag, so a factor that does not divide the grid is bad input. A payload that is not
/// the grid's padded product is not: nothing the caller said produced it.
const fn exit_code_for_table_error(error: &TableError) -> u8 {
    match error {
        TableError::Decimation { .. } | TableError::DecimatedGrid(_) => EXIT_BAD_INPUT,
        TableError::PayloadLength { .. } => EXIT_FAILURE,
    }
}

/// Every disagreement between the file and the declaration is bad input, because the declaration is an
/// argument: `--nodata` and the six grid numbers are what the file is being held to. Bytes that are not
/// there to read are missing data, which is the class that names `mise run data:pull`.
fn exit_code_for_raster_error(error: &RasterError) -> u8 {
    match error {
        RasterError::Dimensions { .. }
        | RasterError::Origin { .. }
        | RasterError::Step { .. }
        | RasterError::Tiled
        | RasterError::SamplesPerPixel { .. }
        | RasterError::PixelFormat { .. }
        | RasterError::RasterType { .. }
        | RasterError::ModelType { .. }
        | RasterError::Epsg { .. }
        | RasterError::ModelTransformation
        | RasterError::NodataMismatch { .. }
        | RasterError::NodataNotANumber { .. }
        | RasterError::NodataMissing { .. }
        | RasterError::MissingTag { .. }
        | RasterError::MissingGeoKey { .. } => EXIT_BAD_INPUT,

        RasterError::UnfetchedPointer => EXIT_MISSING_DATA,
        RasterError::Io(error) if error.kind() == std::io::ErrorKind::NotFound => EXIT_MISSING_DATA,
        RasterError::Io(_) | RasterError::Decode(_) => EXIT_FAILURE,
    }
}

/// A cache that is absent and a cache of some other table share a class, because a caller's answer to
/// both is to build. A cache that is there and broken is neither the caller's doing nor a rebuild away
/// from being trusted, so it is a plain failure and says which file.
fn exit_code_for_cache_error(error: &CacheError) -> u8 {
    match error {
        CacheError::Absent { .. }
        | CacheError::FormatVersion { .. }
        | CacheError::ByteOrderMismatch { .. }
        | CacheError::Digest { .. }
        | CacheError::Width { .. }
        | CacheError::Height { .. }
        | CacheError::DecimationFactor { .. } => EXIT_MISSING_DATA,

        CacheError::HeaderRead { .. }
        | CacheError::HeaderWrite { .. }
        | CacheError::HeaderSyntax { .. }
        | CacheError::PayloadRead { .. }
        | CacheError::PayloadWrite { .. }
        | CacheError::PayloadTruncated { .. }
        | CacheError::PayloadTrailing { .. }
        | CacheError::PayloadAlignment => EXIT_FAILURE,
    }
}

fn exit_code_for_build_error(error: &BuildError<CacheError>) -> u8 {
    match error {
        BuildError::Raster(error) => exit_code_for_raster_error(error),
        BuildError::Sink(error) => exit_code_for_cache_error(error),
    }
}

/// A grid whose columns do not close has no kernels at all, and the six grid numbers are flags, so this is
/// bad input rather than a failure.
const fn exit_code_for_kernel_error(error: KernelError) -> u8 {
    match error {
        KernelError::ColumnsDoNotClose { .. } => EXIT_BAD_INPUT,
    }
}

/// One variant, and it is the kernel's, so a search inherits that classification rather than restating it.
const fn exit_code_for_search_error(error: SearchError) -> u8 {
    match error {
        SearchError::Kernel(error) => exit_code_for_kernel_error(error),
    }
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this
// narrow exemption; docs/ai/code.md allows both in tests.
/// A ledger's grounds split the way [`exit_code_for_cache_error`] splits a cache's, and for that reason: a
/// document describing another table, or one this build reads a different version of, is answered by
/// starting afresh, so it shares the class that names a rebuild. A document that is there and does not
/// hold together — unreadable, not this JSON, or recording two maxima for one radius — is neither the
/// caller's doing nor a rebuild away from being trusted.
///
/// Absent has no arm because it is not an error: opening a [`Ledger`] answers a first run with an empty
/// one.
fn exit_code_for_ledger_error(error: &LedgerError) -> u8 {
    match error {
        LedgerError::FormatVersion { .. }
        | LedgerError::Digest { .. }
        | LedgerError::Width { .. }
        | LedgerError::Height { .. }
        | LedgerError::DecimationFactor { .. } => EXIT_MISSING_DATA,

        LedgerError::Read { .. }
        | LedgerError::Write { .. }
        | LedgerError::Syntax { .. }
        | LedgerError::CentreOffGrid { .. }
        | LedgerError::DuplicateRadius { .. } => EXIT_FAILURE,
    }
}

/// Two arms onto the two layers beneath, so the search over radius invents no class of its own.
fn exit_code_for_smallest_error(error: &SmallestError<LedgerError>) -> u8 {
    match error {
        SmallestError::Search(error) => exit_code_for_search_error(*error),
        SmallestError::Ledger(error) => exit_code_for_ledger_error(error),
    }
}

/// A share through [`Share::from_percent`], so the conversion from percent to fraction is the domain's and
/// this crate divides nothing.
fn parse_share(value: &str) -> Result<Share, String> {
    let percent: u32 = value
        .parse()
        .map_err(|error| format!("`{value}` is not a whole percent: {error}"))?;
    Share::from_percent(percent).map_err(|error| error.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
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

    // One witness per class rather than a table of every variant: unlike `GridError`, these enums do not
    // map uniformly, and what the exhaustive match holds is that a new variant has to be classified
    // before this crate builds. What the tests hold is which class each one is.
    #[test]
    fn a_raster_the_file_contradicts_is_bad_input_and_bytes_that_are_absent_are_missing_data() {
        assert_eq!(
            exit_code_for_raster_error(&RasterError::Tiled),
            EXIT_BAD_INPUT
        );
        assert_eq!(
            exit_code_for_raster_error(&RasterError::UnfetchedPointer),
            EXIT_MISSING_DATA
        );
        assert_eq!(
            exit_code_for_raster_error(&RasterError::Io(std::io::Error::from(
                std::io::ErrorKind::NotFound
            ))),
            EXIT_MISSING_DATA
        );
        assert_eq!(
            exit_code_for_raster_error(&RasterError::Io(std::io::Error::from(
                std::io::ErrorKind::PermissionDenied
            ))),
            EXIT_FAILURE
        );
    }

    #[test]
    fn a_cache_of_another_table_is_missing_data_and_a_broken_one_is_a_failure() {
        assert_eq!(
            exit_code_for_cache_error(&CacheError::Absent {
                path: PathBuf::from("out/table.header.json")
            }),
            EXIT_MISSING_DATA
        );
        assert_eq!(
            exit_code_for_cache_error(&CacheError::Digest {
                expected: 1,
                found: 2
            }),
            EXIT_MISSING_DATA
        );
        assert_eq!(
            exit_code_for_cache_error(&CacheError::PayloadAlignment),
            EXIT_FAILURE
        );
        // Through a build, where the sink's failure is the cache's and keeps its class.
        assert_eq!(
            exit_code_for_build_error(&BuildError::Sink(CacheError::PayloadTruncated {
                expected: 160,
                found: 40
            })),
            EXIT_FAILURE
        );
    }

    #[test]
    fn a_digest_is_read_back_in_the_spelling_a_build_prints() {
        // `report.rs` prints `{:#018x}`, so this is the round trip between the two documents.
        assert_eq!(
            parse_digest("0x3a5d5e3b082f2fb7"),
            Ok(0x3a5d_5e3b_082f_2fb7)
        );
        assert_eq!(parse_digest("3a5d5e3b082f2fb7"), Ok(0x3a5d_5e3b_082f_2fb7));
        assert!(parse_digest("0x not a digest").is_err());
        // The prefix alone is not a digest either, and neither is one 17 digits long.
        assert!(parse_digest("0x").is_err());
        assert!(parse_digest("0x1f17aa802a6890f0c").is_err());
    }

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
    fn a_grid_whose_columns_do_not_close_is_bad_input() {
        assert_eq!(
            exit_code_for_kernel_error(KernelError::ColumnsDoNotClose { lon_span: 90.0 }),
            EXIT_BAD_INPUT
        );
        // A search's only failure is the kernel's, and it keeps the kernel's class.
        assert_eq!(
            exit_code_for_search_error(SearchError::Kernel(KernelError::ColumnsDoNotClose {
                lon_span: 90.0
            })),
            EXIT_BAD_INPUT
        );
    }

    #[test]
    fn a_share_outside_a_proportion_is_refused_by_the_parser() {
        // `Share::from_percent` holds the two grounds, so this crate performs no division and no range
        // check of its own — and the message a caller sees is the domain's.
        assert!(parse_share("0").is_err());
        assert!(parse_share("101").is_err());
        assert!(parse_share("-1").is_err());
        assert!(parse_share("12.5").is_err());
    }

    #[test]
    fn a_share_in_whole_percent_is_the_fraction_a_document_publishes() {
        // Exactly, which is the whole reason the flag is percent rather than a fraction: no accumulated
        // residue reaches a published share.
        assert_eq!(parse_share("50").map(Share::get), Ok(0.5));
        assert_eq!(parse_share("100").map(Share::get), Ok(1.0));
        assert_eq!(parse_share("10").map(Share::get), Ok(0.1));
    }

    #[test]
    fn a_stale_ledger_is_missing_data_and_a_broken_one_is_a_failure() {
        // The split `exit_code_for_cache_error` makes, for its reason: a ledger of another table is
        // answered by starting afresh, and one that does not hold together is not.
        assert_eq!(
            exit_code_for_ledger_error(&LedgerError::Digest {
                expected: 1,
                found: 2
            }),
            EXIT_MISSING_DATA
        );
        assert_eq!(
            exit_code_for_ledger_error(&LedgerError::Syntax {
                path: PathBuf::from("out/radii.json"),
                source: serde_json::from_str::<u32>("nonsense").expect_err("that is not a number"),
            }),
            EXIT_FAILURE
        );
        // Through the search over radius, where a ledger's failure keeps its own class.
        assert_eq!(
            exit_code_for_smallest_error(&SmallestError::Ledger(LedgerError::DecimationFactor {
                expected: 10,
                found: 1
            })),
            EXIT_MISSING_DATA
        );
        assert_eq!(
            exit_code_for_smallest_error(&SmallestError::Search(SearchError::Kernel(
                KernelError::ColumnsDoNotClose { lon_span: 90.0 }
            ))),
            EXIT_BAD_INPUT
        );
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
