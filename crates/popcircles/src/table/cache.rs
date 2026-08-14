// The I/O half of the summation table: the header, the payload and the publication that ties the two.
// ADR 0003 decision 1 keeps every path and every serde derive here, which is what leaves `table.rs` a
// computation its tests reach without a filesystem — the split `raster.rs` and `raster/geotiff.rs`
// already have.
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use memmap2::Mmap;
use serde::{Deserialize, Serialize};

use super::{BuiltTable, Decimation, padded_len};

/// Bumped when a change to the header's fields or the payload's layout is one an older reader would
/// misread rather than refuse.
pub const FORMAT_VERSION: u32 = 1;

const HEADER_SUFFIX: &str = ".header.json";
const PAYLOAD_SUFFIX: &str = ".payload.bin";
/// One suffix for both temporaries, so an interrupted build leaves names the next build can name back.
const TEMPORARY_SUFFIX: &str = ".tmp";

/// The order a payload's cells are in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ByteOrder {
    Little,
    Big,
}

impl ByteOrder {
    /// The order every payload is written in, and the only one [`Cache::read`] accepts. Decision 4:
    /// `bytemuck` casts natively and swaps nothing, so a payload from a host of the other order is
    /// rebuilt rather than reinterpreted, and a format declaring one order while casting in another
    /// would be silently wrong on half the hosts there are.
    pub const HOST: Self = if cfg!(target_endian = "little") {
        Self::Little
    } else {
        Self::Big
    };
}

impl fmt::Display for ByteOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Little => f.write_str("little-endian"),
            Self::Big => f.write_str("big-endian"),
        }
    }
}

/// Every way a cache can fail to be the table a caller asked for.
///
/// One variant per ground, so a refusal says which of them fired: a caller that rebuilds on a digest
/// mismatch and reports a moved payload as an error has to be able to tell the two apart, and a message
/// reading only "stale cache" sends its reader to a hex editor. The same shape, and the same reason, as
/// [`RasterError`](crate::raster::RasterError).
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("no cache at {}: nothing has been published there", path.display())]
    Absent { path: PathBuf },

    #[error("the cache header at {} could not be read", path.display())]
    HeaderRead {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("the cache header at {} could not be written", path.display())]
    HeaderWrite {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("the cache header at {} is not the JSON document this format is", path.display())]
    HeaderSyntax {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("the cache payload at {} could not be read", path.display())]
    PayloadRead {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("the cache payload at {} could not be written", path.display())]
    PayloadWrite {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("this build reads format version {expected}; the header declares {found}")]
    FormatVersion { expected: u32, found: u32 },

    #[error("this host is {expected}; the payload was written {found}, so it has to be rebuilt")]
    ByteOrderMismatch {
        expected: ByteOrder,
        found: ByteOrder,
    },

    #[error(
        "wanted the table of the cells digesting to {expected:#018x}; the header declares \
         {found:#018x}"
    )]
    Digest { expected: u64, found: u64 },

    #[error("wanted a {expected}-column table; the header declares {found}")]
    Width { expected: u32, found: u32 },

    #[error("wanted a {expected}-row table; the header declares {found}")]
    Height { expected: u32, found: u32 },

    #[error("wanted a table decimated by {expected}; the header declares {found}")]
    DecimationFactor { expected: u32, found: u32 },

    #[error("the header describes {expected} payload bytes; the payload stops after {found}")]
    PayloadTruncated { expected: usize, found: usize },

    #[error("the header describes {expected} payload bytes; the payload carries {found}")]
    PayloadTrailing { expected: usize, found: usize },

    // The allocator's doing rather than the file's, and the reason it is a variant at all: a payload
    // read into a `Vec<u8>` is aligned to whatever the allocator returned, which every real one over-
    // aligns and none of them promises. A mapping cannot reach this — it starts at offset 0 of a page.
    #[error(
        "the payload was read into a buffer that is not aligned for {}-byte cells",
        align_of::<f64>()
    )]
    PayloadAlignment,
}

/// What a caller requires of a cache before it will read one: the cells the table was built from, and
/// how coarsely they were folded.
///
/// Both are what a build already produces, so a caller falling back to building has nothing extra to
/// compute, and the grid the table is over travels with the factor rather than beside it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Identity {
    pub digest: u64,
    pub decimation: Decimation,
}

impl From<&BuiltTable> for Identity {
    fn from(built: &BuiltTable) -> Self {
        Self {
            digest: built.digest,
            decimation: built.decimation,
        }
    }
}

/// What the payload beside it is a table of.
///
/// `format_version` is declared first because serde emits struct fields in declaration order, so the
/// field a reader needs before it can trust any other is the first one it meets — the same reason
/// [`Envelope`](crate::report::Envelope) leads with its schema version. The dimensions and the factor
/// sit here rather than inside the digest, per decision 3, so a mismatch in either is reported as
/// itself instead of as an unexplained digest failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct Header {
    format_version: u32,
    digest: u64,
    width: u32,
    height: u32,
    decimation: u32,
    byte_order: ByteOrder,
}

impl Header {
    fn new(identity: &Identity) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            digest: identity.digest,
            width: identity.decimation.grid().width(),
            height: identity.decimation.grid().height(),
            decimation: identity.decimation.factor(),
            byte_order: ByteOrder::HOST,
        }
    }

    /// The version and the byte order come first, and in that order: a header of another version may
    /// mean every field after it differently, and a payload in the other order is not a number this
    /// host can compare against anything.
    fn check(&self, wanted: &Identity) -> Result<(), CacheError> {
        if self.format_version != FORMAT_VERSION {
            return Err(CacheError::FormatVersion {
                expected: FORMAT_VERSION,
                found: self.format_version,
            });
        }
        if self.byte_order != ByteOrder::HOST {
            return Err(CacheError::ByteOrderMismatch {
                expected: ByteOrder::HOST,
                found: self.byte_order,
            });
        }
        if self.digest != wanted.digest {
            return Err(CacheError::Digest {
                expected: wanted.digest,
                found: self.digest,
            });
        }

        let grid = wanted.decimation.grid();
        if self.width != grid.width() {
            return Err(CacheError::Width {
                expected: grid.width(),
                found: self.width,
            });
        }
        if self.height != grid.height() {
            return Err(CacheError::Height {
                expected: grid.height(),
                found: self.height,
            });
        }
        if self.decimation != wanted.decimation.factor() {
            return Err(CacheError::DecimationFactor {
                expected: wanted.decimation.factor(),
                found: self.decimation,
            });
        }
        Ok(())
    }
}

/// The two files a cache is, at one location.
///
/// A header and a payload rather than one self-describing file, per decision 4: a header of any size
/// that is not a page multiple leaves the payload unaligned in a mapping, and a JSON sidecar is
/// something a person debugging a stale cache can read with `cat`.
#[derive(Debug, Clone)]
pub struct Cache {
    header: PathBuf,
    payload: PathBuf,
    header_temporary: PathBuf,
    payload_temporary: PathBuf,
}

impl Cache {
    /// `base` names both files: it is a path with the suffixes this module owns appended, not a
    /// directory and not a file.
    #[must_use]
    pub fn new(base: impl AsRef<Path>) -> Self {
        let base = base.as_ref();
        let header = suffixed(base, HEADER_SUFFIX);
        let payload = suffixed(base, PAYLOAD_SUFFIX);
        Self {
            header_temporary: suffixed(&header, TEMPORARY_SUFFIX),
            payload_temporary: suffixed(&payload, TEMPORARY_SUFFIX),
            header,
            payload,
        }
    }

    #[must_use]
    pub fn header_path(&self) -> &Path {
        &self.header
    }

    #[must_use]
    pub fn payload_path(&self) -> &Path {
        &self.payload
    }

    /// Opens the payload a build writes its rows into.
    ///
    /// # Errors
    /// [`CacheError::PayloadWrite`] when a temporary a previous build left cannot be removed, or the
    /// new one cannot be created.
    pub fn writer(&self) -> Result<Writer<'_>, CacheError> {
        // The temporary names are deterministic, so this is where an interrupted build's leftovers go:
        // at most these two, cleared by the next build rather than accumulating under names nobody can
        // predict.
        for path in [&self.header_temporary, &self.payload_temporary] {
            if let Err(source) = fs::remove_file(path)
                && source.kind() != io::ErrorKind::NotFound
            {
                return Err(CacheError::PayloadWrite {
                    path: path.clone(),
                    source,
                });
            }
        }

        let file =
            File::create(&self.payload_temporary).map_err(|source| CacheError::PayloadWrite {
                path: self.payload_temporary.clone(),
                source,
            })?;
        Ok(Writer {
            cache: self,
            file,
            cells: 0,
        })
    }

    /// The payload behind a header that describes the table `wanted`, mapped rather than read.
    ///
    /// The path for a table that does not fit in memory, which at full resolution is every table:
    /// [`read`](Self::read) materialises 8 bytes a cell and this does not.
    ///
    /// # Errors
    /// [`CacheError::Absent`] when no header is there, and one variant per ground when what is there
    /// is not this table: see [`CacheError`].
    pub fn open(&self, wanted: &Identity) -> Result<Mapped, CacheError> {
        let cells = self.checked_header(wanted)?;
        Mapped::open(&self.payload, cells)
    }

    /// The payload behind a header that describes the table `wanted`, read into memory.
    ///
    /// # Errors
    /// As [`open`](Self::open).
    pub fn read(&self, wanted: &Identity) -> Result<Payload, CacheError> {
        let cells = self.checked_header(wanted)?;
        let bytes = fs::read(&self.payload).map_err(|source| CacheError::PayloadRead {
            path: self.payload.clone(),
            source,
        })?;
        let payload = Payload { bytes, cells };
        // The cast here and not only in the accessor: a payload the header does not describe is a
        // refusal a caller can act on at the point it asked for a table, rather than one arriving at
        // whichever query first looked.
        payload.cells()?;
        Ok(payload)
    }

    /// The header, checked against what the caller wants, and the payload length it therefore implies.
    /// Both read paths go through here, so neither can be the lenient one.
    fn checked_header(&self, wanted: &Identity) -> Result<usize, CacheError> {
        let document = fs::read(&self.header).map_err(|source| {
            if source.kind() == io::ErrorKind::NotFound {
                CacheError::Absent {
                    path: self.header.clone(),
                }
            } else {
                CacheError::HeaderRead {
                    path: self.header.clone(),
                    source,
                }
            }
        })?;
        let header: Header =
            serde_json::from_slice(&document).map_err(|source| CacheError::HeaderSyntax {
                path: self.header.clone(),
                source,
            })?;
        header.check(wanted)?;
        Ok(padded_len(wanted.decimation.grid()))
    }

    /// The commit record, and the last thing publication does: a reader that finds a header finds a
    /// complete payload behind it.
    fn commit_header(&self, identity: &Identity) -> Result<(), CacheError> {
        let document = serde_json::to_vec(&Header::new(identity)).map_err(|source| {
            CacheError::HeaderWrite {
                path: self.header.clone(),
                source: io::Error::other(source),
            }
        })?;
        let write = |path: &PathBuf, source| CacheError::HeaderWrite {
            path: path.clone(),
            source,
        };

        let mut file = File::create(&self.header_temporary)
            .map_err(|source| write(&self.header_temporary, source))?;
        file.write_all(&document)
            .map_err(|source| write(&self.header_temporary, source))?;
        file.sync_all()
            .map_err(|source| write(&self.header_temporary, source))?;
        drop(file);
        fs::rename(&self.header_temporary, &self.header)
            .map_err(|source| write(&self.header, source))
    }
}

/// A build's payload, under a temporary name until [`Writer::publish`] renames it into place.
#[derive(Debug)]
pub struct Writer<'a> {
    cache: &'a Cache,
    file: File,
    cells: usize,
}

impl Writer<'_> {
    /// One completed padded row, in the order [`build`](super::build) emits them.
    ///
    /// # Errors
    /// [`CacheError::PayloadWrite`] when the row cannot be written.
    pub fn write_row(&mut self, row: &[f64]) -> Result<(), CacheError> {
        // f64 to bytes cannot fail — eight divides eight and the alignment falls — and `must_cast_slice`
        // is the form that says so at compile time instead of panicking if it ever stopped being true.
        self.file
            .write_all(bytemuck::must_cast_slice(row))
            .map_err(|source| CacheError::PayloadWrite {
                path: self.cache.payload_temporary.clone(),
                source,
            })?;
        self.cells += row.len();
        Ok(())
    }

    /// Publishes the payload, then the header.
    ///
    /// That order is decision 5's ground, not tidiness: no file is written into in place, so a rename
    /// replaces a directory entry rather than an inode, and a mapping already established keeps the
    /// bytes it mapped while a fresh build publishes over the same path.
    ///
    /// # Errors
    /// [`CacheError::PayloadTruncated`] or [`CacheError::PayloadTrailing`] when the rows written are not
    /// the table `built` describes; [`CacheError::PayloadWrite`] and [`CacheError::HeaderWrite`] when a
    /// sync or a rename fails.
    pub fn publish(self, built: &BuiltTable) -> Result<(), CacheError> {
        let cache = self.cache;
        let identity = Identity::from(built);
        self.commit_payload(&identity)?;
        cache.commit_header(&identity)
    }

    fn commit_payload(self, identity: &Identity) -> Result<(), CacheError> {
        let Self { cache, file, cells } = self;
        // The one thing a writer can be wrong about, refused before a header claiming otherwise is
        // published: neither file is ever trusted to describe the other.
        let expected = payload_bytes(padded_len(identity.decimation.grid()));
        let found = payload_bytes(cells);
        if found < expected {
            return Err(CacheError::PayloadTruncated { expected, found });
        }
        if found > expected {
            return Err(CacheError::PayloadTrailing { expected, found });
        }

        let write = |path: &PathBuf, source| CacheError::PayloadWrite {
            path: path.clone(),
            source,
        };
        file.sync_all()
            .map_err(|source| write(&cache.payload_temporary, source))?;
        drop(file);
        fs::rename(&cache.payload_temporary, &cache.payload)
            .map_err(|source| write(&cache.payload, source))
    }
}

/// A payload mapped into the address space, and the cell count [`Cache::open`] validated.
///
/// It keeps the mapping and not the [`File`]: a mapping outlives the descriptor that created it, so a
/// held descriptor would be a resource nothing reads. It hands out `&[f64]` for
/// [`Table`](super::Table) to borrow, which ties the lifetime of every query to the mapping through the
/// compiler rather than through a rule someone has to remember.
#[derive(Debug)]
pub struct Mapped {
    map: Mmap,
    cells: usize,
}

impl Mapped {
    // The one exception in this workspace: `Cargo.toml` says why `unsafe_code` is `deny` rather than
    // `forbid`, and the `single-unsafe-allow` hook is what asserts this is still the only one. The hook
    // counts the attribute below as text, so naming it a second time anywhere under `crates/` fires it.
    #[allow(unsafe_code)]
    fn open(path: &Path, cells: usize) -> Result<Self, CacheError> {
        let read = |source| CacheError::PayloadRead {
            path: path.to_path_buf(),
            source,
        };
        let file = File::open(path).map_err(read)?;

        // What makes this sound is [`Writer::publish`]'s immutable publication, not this crate's
        // authorship of the file. A rename replaces a directory entry rather than an inode, and no
        // payload is ever written into one already in place, so a fresh build publishing over this path
        // leaves the bytes a mapping already holds untouched. The residual is a third party truncating
        // the cache directory by hand: mmap has no defence against it, the failure is a fault on access
        // rather than a wrong number taken for a right one, and the payload is a derived artefact a
        // build replaces from the raster in about fifteen seconds.
        // SAFETY: no writer this project contains can shrink or rewrite a payload already in place.
        let map = unsafe { Mmap::map(&file) }.map_err(read)?;
        drop(file);

        let mapped = Self { map, cells };
        // The checked cast at open, once, for [`Cache::read`]'s reason.
        mapped.cells()?;
        Ok(mapped)
    }

    /// The mapping as cells, for [`Table::new`](super::Table::new) to borrow.
    ///
    /// # Errors
    /// As [`Payload::cells`], and unreachable for the same reason.
    pub fn cells(&self) -> Result<&[f64], CacheError> {
        view(&self.map, self.cells)
    }
}

/// A payload read into memory, and the cell count the header said it would be.
///
/// It owns bytes rather than cells because the cast is what checks them: an owner of `Vec<f64>` would
/// have had to copy or to cast somewhere it could not report the failure.
#[derive(Debug)]
pub struct Payload {
    bytes: Vec<u8>,
    cells: usize,
}

impl Payload {
    /// The payload as cells, for [`Table::new`](super::Table::new) to borrow.
    ///
    /// # Errors
    /// [`CacheError::PayloadTruncated`], [`CacheError::PayloadTrailing`] or
    /// [`CacheError::PayloadAlignment`] — every one of which [`Cache::read`] has already refused, so a
    /// caller holding a [`Payload`] is asking a question that has been answered.
    pub fn cells(&self) -> Result<&[f64], CacheError> {
        view(&self.bytes, self.cells)
    }
}

/// The one place bytes become cells, so the mapped path and the resident one refuse the same files.
fn view(bytes: &[u8], cells: usize) -> Result<&[f64], CacheError> {
    let expected = payload_bytes(cells);
    if bytes.len() < expected {
        return Err(CacheError::PayloadTruncated {
            expected,
            found: bytes.len(),
        });
    }
    if bytes.len() > expected {
        return Err(CacheError::PayloadTrailing {
            expected,
            found: bytes.len(),
        });
    }
    bytemuck::try_cast_slice(bytes).map_err(|_| CacheError::PayloadAlignment)
}

/// Saturating for [`padded_len`]'s reason: a cell count too large for this host's addresses is a length
/// no payload matches, rather than a smaller one some payload does.
fn payload_bytes(cells: usize) -> usize {
    cells.saturating_mul(size_of::<f64>())
}

fn suffixed(base: &Path, suffix: &str) -> PathBuf {
    let mut path = base.as_os_str().to_owned();
    path.push(suffix);
    PathBuf::from(path)
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this narrow
// exemption; docs/ai/code.md allows both in tests. float_cmp is here because the round trip's claim is
// bit-identity, which a tolerance would not check; cast_precision_loss because the fixture's cells are
// small integers, exact in f32.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::cast_precision_loss
)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::geodesy::LatLon;
    use crate::grid::Grid;
    use crate::raster::Synthetic;
    use crate::table::{ColSpan, RowBand, Table, build};

    /// The registry raster's sentinel, so the fixtures below get the sanitising the real reader does.
    const NODATA: f32 = -3.402_823e38;

    fn grid(width: u32, height: u32) -> Grid {
        Grid::new(
            width,
            height,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            360.0 / f64::from(width),
            -180.0 / f64::from(height),
        )
        .expect("a whole-globe grid of any shape is valid")
    }

    /// Cells no two of which are equal, so a payload written or read at the wrong offset shows up as a
    /// wrong sum rather than as a coincidence.
    fn cells_of(grid: Grid) -> Vec<Vec<f32>> {
        (0..grid.height())
            .map(|row| {
                (0..grid.width())
                    .map(|col| (row * grid.width() + col) as f32 + 0.5)
                    .collect()
            })
            .collect()
    }

    fn source(grid: Grid) -> Synthetic {
        Synthetic::new(grid, NODATA, cells_of(grid)).expect("the fixture is the grid's shape")
    }

    /// Builds a table straight into the cache and commits both files, returning what the build settled.
    fn publish(cache: &Cache, decimation: Decimation) -> BuiltTable {
        let mut writer = cache.writer().unwrap();
        let built = build(source(*decimation.source()), decimation, &mut (), |row| {
            writer.write_row(row)
        })
        .expect("a synthetic source cannot fail and the sink writes to a temporary directory");
        writer.publish(&built).unwrap();
        built
    }

    /// The payload alone, which is what an interrupted publication leaves behind.
    fn publish_payload_only(cache: &Cache, decimation: Decimation) -> BuiltTable {
        let mut writer = cache.writer().unwrap();
        let built = build(source(*decimation.source()), decimation, &mut (), |row| {
            writer.write_row(row)
        })
        .expect("a synthetic source cannot fail and the sink writes to a temporary directory");
        writer.commit_payload(&Identity::from(&built)).unwrap();
        built
    }

    fn cache_in(directory: &TempDir) -> Cache {
        Cache::new(directory.path().join("gpw-v4"))
    }

    fn header_of(built: &BuiltTable) -> Header {
        Header::new(&Identity::from(built))
    }

    fn write_header(cache: &Cache, header: &Header) {
        fs::write(cache.header_path(), serde_json::to_vec(header).unwrap()).unwrap();
    }

    /// Every rectangle of a table, in one order: two of these are equal only if the tables agree over
    /// the whole grid, wrapped spans and full turns included.
    fn every_rectangle(table: &Table<'_>) -> Vec<f64> {
        let grid = *table.grid();
        let mut sums = Vec::new();
        for north in 0..grid.height() {
            for south in north..grid.height() {
                let band = RowBand::new(grid.row(north).unwrap(), grid.row(south).unwrap());
                sums.push(table.population(band, ColSpan::FullTurn));
                for west in 0..grid.width() {
                    for east in 0..grid.width() {
                        let span = ColSpan::Through {
                            west: grid.col(west).unwrap(),
                            east: grid.col(east).unwrap(),
                        };
                        sums.push(table.population(band, span));
                    }
                }
            }
        }
        sums
    }

    fn other_order() -> ByteOrder {
        match ByteOrder::HOST {
            ByteOrder::Little => ByteOrder::Big,
            ByteOrder::Big => ByteOrder::Little,
        }
    }

    #[test]
    fn the_header_leads_with_its_format_version() {
        // On the text rather than on a parsed value, for `report.rs`'s reason: what needs pinning is
        // that a reader streaming the document meets the version before anything it might not
        // understand.
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        publish(&cache, Decimation::none(grid(4, 3)));
        let document = fs::read_to_string(cache.header_path()).unwrap();
        assert!(
            document.starts_with(r#"{"format_version":1,"#),
            "{document}"
        );
    }

    #[test]
    fn every_rectangle_survives_the_round_trip() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let decimation = Decimation::none(grid(5, 4));

        let mut as_built = Vec::new();
        let mut writer = cache.writer().unwrap();
        let built = build(source(*decimation.source()), decimation, &mut (), |row| {
            as_built.extend_from_slice(row);
            writer.write_row(row)
        })
        .unwrap();
        writer.publish(&built).unwrap();

        let payload = cache.read(&Identity::from(&built)).unwrap();
        let reloaded = Table::new(*decimation.grid(), payload.cells().unwrap()).unwrap();
        let original = Table::new(*decimation.grid(), &as_built).unwrap();

        // Bit-identical, not close: a payload is bytes this host wrote and bytes this host read, so
        // anything but equality is a byte that moved.
        assert_eq!(every_rectangle(&reloaded), every_rectangle(&original));
    }

    #[test]
    fn the_mapped_payload_and_the_resident_one_are_the_same_table() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let decimation = Decimation::none(grid(5, 4));
        let built = publish(&cache, decimation);
        let wanted = Identity::from(&built);

        let resident = cache.read(&wanted).unwrap();
        let mapped = cache.open(&wanted).unwrap();
        // The claim the `unsafe` has to earn: the two differ in how the bytes reach the query and in
        // nothing else, over the same pair of files.
        assert_eq!(
            every_rectangle(&Table::new(*decimation.grid(), mapped.cells().unwrap()).unwrap()),
            every_rectangle(&Table::new(*decimation.grid(), resident.cells().unwrap()).unwrap())
        );
    }

    #[test]
    fn a_digest_from_other_cells_is_refused() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        let mut header = header_of(&built);
        header.digest ^= 1;
        write_header(&cache, &header);

        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::Digest { expected, found })
                if expected == built.digest && found == built.digest ^ 1
        ));
    }

    #[test]
    fn a_width_that_is_not_the_tables_is_refused() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        let mut header = header_of(&built);
        header.width += 1;
        write_header(&cache, &header);

        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::Width {
                expected: 4,
                found: 5
            })
        ));
    }

    #[test]
    fn a_height_that_is_not_the_tables_is_refused() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        let mut header = header_of(&built);
        header.height += 1;
        write_header(&cache, &header);

        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::Height {
                expected: 3,
                found: 4
            })
        ));
    }

    #[test]
    fn a_decimation_factor_that_is_not_the_tables_is_refused() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        let mut header = header_of(&built);
        header.decimation = 2;
        write_header(&cache, &header);

        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::DecimationFactor {
                expected: 1,
                found: 2
            })
        ));
    }

    #[test]
    fn a_format_version_from_another_release_is_refused() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        let mut header = header_of(&built);
        header.format_version = FORMAT_VERSION + 1;
        // The digest goes too, and the refusal still names the version: a header of another version is
        // not a document whose other fields this build knows how to compare.
        header.digest ^= 1;
        write_header(&cache, &header);

        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::FormatVersion { expected, found })
                if expected == FORMAT_VERSION && found == FORMAT_VERSION + 1
        ));
    }

    #[test]
    fn a_payload_from_a_host_of_the_other_byte_order_is_refused() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        let mut header = header_of(&built);
        header.byte_order = other_order();
        write_header(&cache, &header);

        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::ByteOrderMismatch { expected, found })
                if expected == ByteOrder::HOST && found == other_order()
        ));
    }

    #[test]
    fn a_payload_truncated_mid_element_is_refused() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        let mut bytes = fs::read(cache.payload_path()).unwrap();
        let full = bytes.len();
        bytes.truncate(full - 3);
        fs::write(cache.payload_path(), &bytes).unwrap();

        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::PayloadTruncated { expected, found })
                if expected == full && found == full - 3
        ));
    }

    #[test]
    fn a_payload_carrying_trailing_bytes_is_refused() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        let mut bytes = fs::read(cache.payload_path()).unwrap();
        let full = bytes.len();
        bytes.extend_from_slice(&[0, 0, 0]);
        fs::write(cache.payload_path(), &bytes).unwrap();

        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::PayloadTrailing { expected, found })
                if expected == full && found == full + 3
        ));
    }

    #[test]
    fn a_header_that_is_not_json_is_refused() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        fs::write(cache.header_path(), b"{\"format_version\": ").unwrap();

        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::HeaderSyntax { .. })
        ));
    }

    #[test]
    fn a_header_whose_dimensions_disagree_with_the_payload_is_refused() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let built = publish(&cache, Decimation::none(grid(4, 3)));

        // The header and the caller agree on a wider table than the payload holds, so neither the
        // dimensions nor the digest catch it and only the payload's own length can.
        let wanted = Identity {
            digest: built.digest,
            decimation: Decimation::none(grid(6, 3)),
        };
        write_header(&cache, &Header::new(&wanted));

        assert!(matches!(
            cache.read(&wanted),
            Err(CacheError::PayloadTruncated {
                expected: 224,
                found: 160
            })
        ));
    }

    #[test]
    fn a_payload_published_under_an_earlier_builds_header_is_not_that_table() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let first = publish(&cache, Decimation::none(grid(4, 3)));
        // A second build's payload renamed into place while the first build's header is still there:
        // the state the publication order leaves for as long as it takes to write the header, and the
        // reason the header is the commit record rather than the payload.
        publish_payload_only(&cache, Decimation::none(grid(6, 3)));

        assert!(matches!(
            cache.read(&Identity::from(&first)),
            Err(CacheError::PayloadTrailing {
                expected: 160,
                found: 224
            })
        ));
    }

    #[test]
    fn an_interrupted_publication_is_a_missing_cache_and_its_orphan_is_cleared() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let decimation = Decimation::none(grid(4, 3));
        let built = publish_payload_only(&cache, decimation);

        // Missing rather than corrupt: nothing has committed a header, so there is no table to be
        // wrong about, and a caller's answer is to build.
        assert!(matches!(
            cache.read(&Identity::from(&built)),
            Err(CacheError::Absent { .. })
        ));

        // And the other half of an interruption — a temporary that never got renamed — is cleared by
        // the next build rather than left for a directory to accumulate.
        let orphan = cache.payload_temporary.clone();
        fs::write(&orphan, b"a payload a crashed build was part way through").unwrap();
        let writer = cache.writer().unwrap();
        assert_eq!(fs::metadata(&orphan).unwrap().len(), 0);
        drop(writer);

        publish(&cache, decimation);
        assert!(!orphan.exists());
        assert!(cache.read(&Identity::from(&built)).is_ok());
    }

    #[test]
    fn a_writer_refuses_to_publish_a_payload_of_another_shape() {
        let directory = TempDir::new().unwrap();
        let cache = cache_in(&directory);
        let decimation = Decimation::none(grid(4, 3));

        let built = build(source(*decimation.source()), decimation, &mut (), |_row| {
            Ok::<(), CacheError>(())
        })
        .unwrap();

        let mut writer = cache.writer().unwrap();
        writer.write_row(&[0.0; 5]).unwrap();

        assert!(matches!(
            writer.publish(&built),
            Err(CacheError::PayloadTruncated {
                expected: 160,
                found: 40
            })
        ));
    }
}
