// The four search commands run for real, against a cache this file builds. What only a real invocation
// can check is here and nothing else: that stdout is one JSON document, that stderr stays out of it, that
// the exit code is the class the failure falls in, and that the document names the table it was answered
// from. The wire format itself is snapshotted in the library, over `report`'s own types — see ADR 0001
// decision 3, and `FU-03`, whose tripwire watches `crates/popcircles/src/snapshots/` and would not see a
// snapshot kept here.
//
// No raster anywhere. `Synthetic` is public and unconditional, so the fixture streams into a real cache
// through the writer a build uses, and the suite passes on a clone that has fetched no LFS content.

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this narrow
// exemption; docs/ai/code.md allows both in tests. cast_precision_loss likewise — the largest cell is 648,
// which u32 -> f32 holds exactly.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::cast_precision_loss)]

use std::path::PathBuf;
use std::process::{Command, Output};

use popcircles::geodesy::LatLon;
use popcircles::grid::Grid;
use popcircles::raster::Synthetic;
use popcircles::table::cache::Cache;
use popcircles::table::{Decimation, build};
use tempfile::TempDir;

const WIDTH: u32 = 36;
const HEIGHT: u32 = 18;

/// The registry raster's sentinel, so the fixture reaches the table by the path a real raster takes.
const NODATA: f32 = -3.402_823e38;

/// The library's spelling of a digest, which is what a build prints and a flag takes back.
fn hexadecimal(digest: u64) -> String {
    format!("{digest:#018x}")
}

/// A cache built from a synthetic raster, under a directory that goes away with the test.
struct Fixture {
    directory: TempDir,
    digest: u64,
}

impl Fixture {
    /// Ten degrees a side and closing in longitude, which is `report.rs`'s fixture and `circle.rs`'s: the
    /// smallest whole-globe shape a kernel accepts.
    fn grid() -> Grid {
        Grid::new(
            WIDTH,
            HEIGHT,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            10.0,
            -10.0,
        )
        .expect("a 36 x 18 whole-globe grid is valid")
    }

    fn build() -> Self {
        let directory = TempDir::new().expect("a temporary directory");
        let grid = Self::grid();
        let rows: Vec<Vec<f32>> = (0..HEIGHT)
            .map(|row| {
                (0..WIDTH)
                    .map(|col| (row * WIDTH + col + 1) as f32)
                    .collect()
            })
            .collect();
        let source = Synthetic::new(grid, NODATA, rows).expect("the rows are the grid's shape");

        let cache = Cache::new(directory.path().join("table"));
        let mut writer = cache.writer().expect("a writer under a fresh directory");
        let built = build(source, Decimation::none(grid), &mut (), |row| {
            writer.write_row(row)
        })
        .expect("a synthetic source and a fresh cache cannot fail");
        writer.publish(&built).expect("the cache publishes");

        Self {
            directory,
            digest: built.digest,
        }
    }

    fn cache_base(&self) -> PathBuf {
        self.directory.path().join("table")
    }

    fn ledger(&self, name: &str) -> PathBuf {
        self.directory.path().join(name)
    }

    /// The ten flags every command reading this cache takes, with `digest` so a case can pass one that
    /// names another table.
    ///
    /// `--log-level error` because `info` is the default and narrates: it is what keeps the success cases
    /// silent, and with it `one_document_naming_the_fixture`'s emptiness assertion is strictly stronger
    /// than dropping the assertion would be. `narration_follows_the_level_and_stdout_does_not` is where
    /// the narration itself is checked.
    fn flags(&self, digest: &str) -> Vec<String> {
        self.flags_at(digest, "error")
    }

    /// The same at a level the caller names. Substituted rather than appended, because a global argument is
    /// refused a second occurrence and no case wants two.
    fn flags_at(&self, digest: &str, level: &str) -> Vec<String> {
        [
            "--width",
            "36",
            "--height",
            "18",
            "--origin-lat",
            "90",
            "--origin-lon",
            "-180",
            "--lon-step",
            "10",
            "--lat-step",
            "-10",
        ]
        .iter()
        .map(|flag| (*flag).to_string())
        .chain([
            "--cache".to_string(),
            self.cache_base().to_string_lossy().into_owned(),
            "--digest".to_string(),
            digest.to_string(),
            "--log-level".to_string(),
            level.to_string(),
        ])
        .collect()
    }

    fn run(&self, command: &str, digest: &str, rest: &[&str]) -> Output {
        run_with(&self.flags(digest), command, rest)
    }

    /// The command with this fixture's own digest, which is the case every success below is.
    fn run_ok(&self, command: &str, rest: &[&str]) -> Output {
        self.run(command, &hexadecimal(self.digest), rest)
    }

    /// The command with this fixture's own digest, at a level of the caller's choosing.
    fn run_at_level(&self, level: &str, command: &str, rest: &[&str]) -> Output {
        let flags = self.flags_at(&hexadecimal(self.digest), level);
        run_with(&flags, command, rest)
    }
}

fn run_with(flags: &[String], command: &str, rest: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_popcircles-cli"))
        .arg(command)
        .args(flags)
        .args(rest)
        .output()
        .expect("the binary cargo just built runs")
}

/// What every success case asserts, and it is the whole of what running the binary adds over a unit test:
/// stdout is exactly one JSON document, stderr is empty so stdout stays machine-readable, the exit code is
/// zero, and `provenance.digest` names the table the fixture built.
///
/// The emptiness now holds because [`Fixture::flags`] asks for `error`, where it used to hold because
/// nothing emitted. What moved is the reason, not the claim.
fn one_document_naming_the_fixture(fixture: &Fixture, output: &Output) -> serde_json::Value {
    assert!(
        output.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");

    // `from_slice` refuses trailing content, so a second document on stdout fails here rather than being
    // read as the first.
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is one JSON document");
    assert_eq!(document["schema_version"], 1);
    assert_eq!(
        document["provenance"]["digest"],
        serde_json::Value::from(hexadecimal(fixture.digest))
    );
    document
}

#[test]
fn population_at_answers_from_the_cache() {
    let fixture = Fixture::build();
    let output = fixture.run_ok(
        "population-at",
        &["--lat", "48", "--lon", "11", "--radius-km", "1200"],
    );
    let document = one_document_naming_the_fixture(&fixture, &output);
    // The figure `report.rs`'s snapshot pins, reached this time through the parser and a real cache.
    assert_eq!(document["result"]["population"], 820.0);
}

#[test]
fn most_populous_answers_from_the_cache() {
    let fixture = Fixture::build();
    let output = fixture.run_ok("most-populous", &["--radius-km", "3000", "--spacing", "4"]);
    let document = one_document_naming_the_fixture(&fixture, &output);
    assert!(
        document["result"]["stats"]["blocks_pruned"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
fn smallest_for_share_answers_from_the_cache_and_writes_its_ledger() {
    let fixture = Fixture::build();
    let ledger = fixture.ledger("radii.json");
    let output = fixture.run_ok(
        "smallest-for-share",
        &[
            "--share",
            "25",
            "--spacing",
            "4",
            "--ledger",
            &ledger.to_string_lossy(),
        ],
    );
    let document = one_document_naming_the_fixture(&fixture, &output);
    // The ledger block is a claim about a file, so the file is what checks it.
    assert!(ledger.is_file(), "the ledger was not written");
    assert!(document["result"]["ledger"]["radii"].as_u64().unwrap() > 0);
}

#[test]
fn sweep_answers_every_share_over_one_ledger() {
    let fixture = Fixture::build();
    let ledger = fixture.ledger("sweep-radii.json");
    let output = fixture.run_ok(
        "sweep",
        &[
            "--from",
            "10",
            "--to",
            "30",
            "--step",
            "10",
            "--spacing",
            "4",
            "--ledger",
            &ledger.to_string_lossy(),
        ],
    );
    let document = one_document_naming_the_fixture(&fixture, &output);
    let records = document["result"]["records"]
        .as_array()
        .expect("the sweep publishes records");
    assert_eq!(records.len(), 3);
    // Ascending and exact, which is the contract `SweepDocument::new` holds and the reason for the percent
    // flags — read here off a document a real invocation produced.
    let shares: Vec<f64> = records
        .iter()
        .map(|record| record["target"]["share"].as_f64().unwrap())
        .collect();
    assert_eq!(shares, vec![0.1, 0.2, 0.3]);
}

/// One invocation at two levels, which is what makes the flag rather than the code path the thing being
/// checked.
#[test]
fn narration_follows_the_level_and_stdout_does_not() {
    let fixture = Fixture::build();
    let circle = ["--lat", "48", "--lon", "11", "--radius-km", "1200"];

    let narrated = fixture.run_at_level("info", "population-at", &circle);
    assert_eq!(narrated.status.code(), Some(0), "{narrated:?}");
    let narration = String::from_utf8_lossy(&narrated.stderr);
    // Box 6's two ends and nothing between them: the table that answered, then the answer.
    assert_eq!(narration.lines().count(), 2, "{narration}");
    assert!(narration.contains("opened from"), "{narration}");
    assert!(narration.contains("holds 820"), "{narration}");

    let silent = fixture.run_at_level("warn", "population-at", &circle);
    assert_eq!(silent.status.code(), Some(0), "{silent:?}");
    assert!(
        silent.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&silent.stderr)
    );

    // Byte for byte, which is the property box 6 exists to protect: the level governs stderr and reaches
    // the document not at all.
    assert_eq!(narrated.stdout, silent.stdout);
}

#[test]
fn a_digest_naming_another_table_is_missing_data_and_prints_nothing() {
    let fixture = Fixture::build();
    let output = fixture.run(
        "population-at",
        "0x0000000000000001",
        &["--lat", "48", "--lon", "11", "--radius-km", "1200"],
    );
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    // Nothing on stdout, which is what makes a consumer's parse failure impossible rather than confusing.
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(!output.stderr.is_empty(), "a failure says why");
}

#[test]
fn a_coordinate_off_the_grid_is_bad_input() {
    let fixture = Fixture::build();
    // The outer southern boundary lies in no cell, and the cache opens cleanly first — so this is the
    // coordinate being refused rather than the table.
    let output = fixture.run_ok(
        "population-at",
        &["--lat", "-90", "--lon", "0", "--radius-km", "1200"],
    );
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
}
