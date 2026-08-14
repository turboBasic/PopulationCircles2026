// The I/O half of the search over radius: one JSON document holding what every probe found, so an
// interrupted run resumes instead of paying for its radii twice. ADR 0003 decision 1's split, at a tenth of
// the size — `smallest.rs` stays a computation whose tests need no filesystem, and every path, every
// `serde` derive and every write in this work is here.
//
// One document rather than the header-and-payload pair `table/cache.rs` publishes. That shape exists so a
// 7.5 GB payload can be mapped with its cells aligned; a ledger holds a few dozen rows, is read once at
// startup, and is something a person debugging a resumed run wants to be able to `cat`. What it keeps from
// that record is the two things that make a cache safe rather than fast: the identity it is checked
// against, and publication by rename so a reader never sees a half-written document.
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::RadiusLedger;
use crate::grid::Grid;
use crate::search::Candidate;
use crate::table::cache::Identity;

/// Bumped when a change to the document's fields is one an older reader would misread rather than refuse.
///
/// Additive growth bumps it too, unlike a wire format's version: `serde` ignores keys it does not know, so
/// a build reading a later document would accept it and resume from radii it has half understood. That is
/// `FU-06`'s reasoning for the table's header, and a ledger is the same kind of file.
pub const FORMAT_VERSION: u32 = 1;

/// One suffix, so an interrupted publication leaves a name the next one can name back.
const TEMPORARY_SUFFIX: &str = ".tmp";

/// Every way a ledger can fail to be the one a caller asked for.
///
/// One variant per ground, [`CacheError`](crate::table::cache::CacheError)'s reasoning: a caller that
/// starts afresh on a digest mismatch and reports a broken file as an error has to be able to tell the two
/// apart, and a message reading only "stale ledger" sends its reader to a hex editor.
#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error("the ledger at {} could not be read", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("the ledger at {} could not be written", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("the ledger at {} is not the JSON document this format is", path.display())]
    Syntax {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("this build reads format version {expected}; the ledger declares {found}")]
    FormatVersion { expected: u32, found: u32 },

    #[error(
        "wanted the radii of the table whose cells digest to {expected:#018x}; the ledger declares \
         {found:#018x}"
    )]
    Digest { expected: u64, found: u64 },

    #[error("wanted a {expected}-column table; the ledger declares {found}")]
    Width { expected: u32, found: u32 },

    #[error("wanted a {expected}-row table; the ledger declares {found}")]
    Height { expected: u32, found: u32 },

    #[error("wanted a table decimated by {expected}; the ledger declares {found}")]
    DecimationFactor { expected: u32, found: u32 },

    #[error(
        "the ledger records a maximum at ({row}, {col}), which is not a cell of a {width} x {height} grid"
    )]
    CentreOffGrid {
        row: u32,
        col: u32,
        width: u32,
        height: u32,
    },

    #[error("the ledger records {km} km twice, with maxima of {first} and {second}")]
    DuplicateRadius { km: u32, first: f64, second: f64 },
}

/// One probe, as published.
///
/// The centre is a pair of indices rather than a [`Candidate`]: a [`Row`](crate::grid::Row) is only ever
/// minted by the grid that contains it, so the indices are checked back into cells at open time and no
/// later reader has to wonder whether they are on the grid.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct Probe {
    km: u32,
    population: f64,
    row: u32,
    col: u32,
}

/// The document.
///
/// `format_version` is declared first because serde emits struct fields in declaration order, so the field
/// a reader needs before it can trust any other is the first one it meets — the table's header and the
/// report envelope both lead with theirs for the same reason. The digest, the dimensions and the factor are
/// [`Identity`]'s, spelled out rather than nested so a mismatch in any one of them is reported as itself.
///
/// **The share is not here, and neither is the spacing.** What a row records is the maximum over the
/// table's cell centres at a radius, which is a property of the table alone: keying on the spacing would
/// evict a ledger a different spacing filled, and keying on the share would stop a rerun at 25% reusing
/// what a 50% run paid for.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Document {
    format_version: u32,
    digest: u64,
    width: u32,
    height: u32,
    decimation: u32,
    radii: Vec<Probe>,
}

impl Document {
    fn new(identity: &Identity) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            digest: identity.digest,
            width: identity.decimation.grid().width(),
            height: identity.decimation.grid().height(),
            decimation: identity.decimation.factor(),
            radii: Vec::new(),
        }
    }

    /// The version comes first, and for the table header's reason: a document of another version may mean
    /// every field after it differently, so nothing else is compared until it agrees.
    fn check(&self, wanted: &Identity) -> Result<(), LedgerError> {
        if self.format_version != FORMAT_VERSION {
            return Err(LedgerError::FormatVersion {
                expected: FORMAT_VERSION,
                found: self.format_version,
            });
        }
        if self.digest != wanted.digest {
            return Err(LedgerError::Digest {
                expected: wanted.digest,
                found: self.digest,
            });
        }

        let grid = wanted.decimation.grid();
        if self.width != grid.width() {
            return Err(LedgerError::Width {
                expected: grid.width(),
                found: self.width,
            });
        }
        if self.height != grid.height() {
            return Err(LedgerError::Height {
                expected: grid.height(),
                found: self.height,
            });
        }
        if self.decimation != wanted.decimation.factor() {
            return Err(LedgerError::DecimationFactor {
                expected: wanted.decimation.factor(),
                found: self.decimation,
            });
        }
        Ok(())
    }
}

/// What every probe of a search over radius found, kept in a file between runs.
///
/// The entries are held resident and the file is rewritten whole on every `put`. Both follow from the size:
/// a search settles a few dozen radii, so the map is kilobytes and the rewrite is one small write against a
/// probe that costs a search over the globe.
#[derive(Debug)]
pub struct Ledger {
    path: PathBuf,
    temporary: PathBuf,
    identity: Identity,
    grid: Grid,
    entries: BTreeMap<u32, Candidate>,
}

impl Ledger {
    /// The ledger at `path` for the table `wanted` describes, or an empty one when nothing is there.
    ///
    /// Absent is not an error — a first run has nothing to resume, and making a caller distinguish "no file
    /// yet" from "wrong file" at every call site is how one of the two ends up handled wrongly. Every other
    /// disagreement is refused by ground: see [`LedgerError`].
    ///
    /// # Errors
    /// [`LedgerError::Read`] when the file is there and cannot be read, [`LedgerError::Syntax`] when it is
    /// not this document, and one variant per ground when it describes another table.
    pub fn open_or_empty(path: impl AsRef<Path>, wanted: &Identity) -> Result<Self, LedgerError> {
        let path = path.as_ref().to_path_buf();
        let mut temporary = path.clone().into_os_string();
        temporary.push(TEMPORARY_SUFFIX);
        let grid = *wanted.decimation.grid();

        let mut ledger = Self {
            temporary: PathBuf::from(temporary),
            path,
            identity: *wanted,
            grid,
            entries: BTreeMap::new(),
        };

        let bytes = match fs::read(&ledger.path) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(ledger),
            Err(source) => {
                return Err(LedgerError::Read {
                    path: ledger.path,
                    source,
                });
            }
        };

        let document: Document =
            serde_json::from_slice(&bytes).map_err(|source| LedgerError::Syntax {
                path: ledger.path.clone(),
                source,
            })?;
        document.check(wanted)?;

        for probe in document.radii {
            let cell = ledger.grid.row(probe.row).zip(ledger.grid.col(probe.col));
            let (row, col) = cell.ok_or(LedgerError::CentreOffGrid {
                row: probe.row,
                col: probe.col,
                width: ledger.grid.width(),
                height: ledger.grid.height(),
            })?;
            let found = Candidate {
                row,
                col,
                population: probe.population,
            };
            // A radius has one maximum. Two rows for it are two answers to the same question, and a reader
            // that took the last would resume from whichever the writer happened to append second.
            if let Some(first) = ledger.entries.insert(probe.km, found) {
                return Err(LedgerError::DuplicateRadius {
                    km: probe.km,
                    first: first.population,
                    second: probe.population,
                });
            }
        }
        Ok(ledger)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// How many radii the file holds, which is what a resumed run reuses.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Writes the whole document to a temporary, syncs it, and renames it into place.
    ///
    /// ADR 0003 decision 5's publication, and the reason is the same: a rename replaces a directory entry
    /// rather than an inode, so a reader either sees the document that was there before or the one this
    /// call finished, never the half-written middle. The temporary's name is deterministic, so an
    /// interrupted run leaves at most one and the next `put` names it back rather than accumulating.
    fn publish(&self) -> Result<(), LedgerError> {
        let mut document = Document::new(&self.identity);
        document.radii = self
            .entries
            .iter()
            .map(|(&km, found)| Probe {
                km,
                population: found.population,
                row: found.row.get(),
                col: found.col.get(),
            })
            .collect();

        let write = |path: &Path, source| LedgerError::Write {
            path: path.to_path_buf(),
            source,
        };
        let bytes = serde_json::to_vec(&document)
            .map_err(|source| write(&self.path, io::Error::other(source)))?;

        let mut file =
            File::create(&self.temporary).map_err(|source| write(&self.temporary, source))?;
        file.write_all(&bytes)
            .map_err(|source| write(&self.temporary, source))?;
        file.sync_all()
            .map_err(|source| write(&self.temporary, source))?;
        drop(file);
        fs::rename(&self.temporary, &self.path).map_err(|source| write(&self.path, source))
    }
}

impl RadiusLedger for Ledger {
    type Error = LedgerError;

    fn get(&self, km: u32) -> Option<Candidate> {
        self.entries.get(&km).copied()
    }

    /// The entry is added and the document republished before this returns, which is what
    /// [`RadiusLedger::put`] promises and what makes an interrupted run resumable: a probe that has been
    /// paid for is on disk before the next one starts.
    fn put(&mut self, km: u32, found: Candidate) -> Result<(), Self::Error> {
        self.entries.insert(km, found);
        self.publish()
    }
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this narrow
// exemption; docs/ai/code.md allows both in tests. float_cmp is here because the round trip's claim is
// bit-identity, which a tolerance would not check.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::geodesy::LatLon;
    use crate::table::Decimation;

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

    const DIGEST: u64 = 0x3a5d_5e3b_082f_2fb7;

    fn identity(width: u32, height: u32) -> Identity {
        Identity {
            digest: DIGEST,
            decimation: Decimation::none(grid(width, height)),
        }
    }

    fn candidate(identity: &Identity, row: u32, col: u32, population: f64) -> Candidate {
        let grid = identity.decimation.grid();
        Candidate {
            row: grid.row(row).expect("a row of the fixture"),
            col: grid.col(col).expect("a column of the fixture"),
            population,
        }
    }

    fn ledger_in(directory: &TempDir, identity: &Identity) -> Ledger {
        Ledger::open_or_empty(directory.path().join("radii.json"), identity).unwrap()
    }

    /// Three probes with distinct populations, so a row read back at the wrong offset shows up as a wrong
    /// maximum rather than as a coincidence.
    fn fill(ledger: &mut Ledger, identity: &Identity) {
        for (km, row, col, population) in [
            (0u32, 0u32, 0u32, 1.5f64),
            (1571, 2, 3, 300.25),
            (1572, 2, 3, 400.5),
        ] {
            ledger
                .put(km, candidate(identity, row, col, population))
                .unwrap();
        }
    }

    #[test]
    fn a_ledger_where_nothing_exists_is_empty_and_its_first_put_creates_the_file() {
        let directory = TempDir::new().unwrap();
        let identity = identity(4, 3);
        let mut ledger = ledger_in(&directory, &identity);

        // Absent rather than an error, because a first run has nothing to resume.
        assert!(ledger.is_empty());
        assert_eq!(ledger.len(), 0);
        assert!(!ledger.path().exists());

        ledger.put(500, candidate(&identity, 1, 2, 42.0)).unwrap();
        assert!(ledger.path().exists());
        assert_eq!(ledger.len(), 1);
    }

    #[test]
    fn the_document_leads_with_its_format_version() {
        // On the text rather than on a parsed value, for `table/cache.rs`'s reason: what needs pinning is
        // that a reader streaming the document meets the version before anything it might not understand.
        let directory = TempDir::new().unwrap();
        let identity = identity(4, 3);
        let mut ledger = ledger_in(&directory, &identity);
        ledger.put(500, candidate(&identity, 1, 2, 42.0)).unwrap();

        let document = fs::read_to_string(ledger.path()).unwrap();
        assert!(
            document.starts_with(r#"{"format_version":1,"#),
            "{document}"
        );
    }

    #[test]
    fn every_probe_survives_the_round_trip() {
        let directory = TempDir::new().unwrap();
        let identity = identity(8, 6);
        let mut written = ledger_in(&directory, &identity);
        fill(&mut written, &identity);

        let reopened = ledger_in(&directory, &identity);
        assert_eq!(reopened.len(), 3);
        for km in [0u32, 1571, 1572] {
            let before = written.get(km).expect("a radius that was put");
            let after = reopened.get(km).expect("a radius that was written");
            // Bit-identical, not close: these are bytes this host wrote and bytes this host read, so
            // anything but equality is a figure that moved.
            assert_eq!(after.population.to_bits(), before.population.to_bits());
            assert_eq!(after.row, before.row);
            assert_eq!(after.col, before.col);
        }
        // And a radius nobody probed is still absent after a reopen.
        assert_eq!(reopened.get(1570), None);
    }
}
