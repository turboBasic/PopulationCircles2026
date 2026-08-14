use std::fs;
use std::io::{IsTerminal, Write};
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
    CircleReport, DistanceReport, Envelope, GridSummary, Provenance, TableBuildReport,
    TableQueryReport,
};
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

    // Resolving where the cache goes, and making room for it, is the shell's work — the library is
    // handed a path and never asked where one should be.
    let cache = Cache::new(&table.cache);
    if let Some(parent) = table.cache.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| Failure {
            code: EXIT_FAILURE,
            message: format!(
                "the cache directory {} could not be made: {error}",
                parent.display()
            ),
        })?;
    }

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
    grid.cell_containing(at).ok_or_else(|| Failure {
        code: EXIT_BAD_INPUT,
        message: format!(
            "(lat {}, lon {}) is not on a {} x {} grid whose origin is (lat {}, lon {}); a coordinate \
             on the grid's outer southern or eastern boundary lies in no cell",
            at.lat,
            at.lon,
            grid.width(),
            grid.height(),
            grid.origin().lat,
            grid.origin().lon
        ),
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

fn query_table(cached: &CachedTableArgs, window: Option<Window>) -> Result<String, Failure> {
    let cached = CachedTable::open(cached)?;
    let view = cached.table()?;
    let grid = cached.grid;

    let (rows, cols) = match window {
        Some(window) => view.covering(window).ok_or_else(|| Failure {
            code: EXIT_BAD_INPUT,
            message: format!(
                "the window is not on a {} x {} grid whose origin is (lat {}, lon {}); a coordinate \
                 on the grid's outer southern or eastern boundary lies in no cell, and the whole \
                 extent is what the query does with no window at all",
                grid.width(),
                grid.height(),
                grid.origin().lat,
                grid.origin().lon
            ),
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

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this
// narrow exemption; docs/ai/code.md allows both in tests.
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
