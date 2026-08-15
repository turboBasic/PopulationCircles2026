import lint_version_bumps as lvb

SNAPSHOT = """---
source: crates/popcircles/src/report.rs
expression: "Envelope::new(GridSummary::from(&grid))"
---
{
  "schema_version": 1,
  "result": {
    "width": 4,
    "height": 2
  }
}
"""

CACHE = """/// Bumped when the header's fields move.
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Header {
    format_version: u32,
    digest: u64,
    #[serde(default)]
    width: u32,
}

impl Header {
    fn new() -> Self {
        Self { format_version: FORMAT_VERSION, digest: 0, width: 0 }
    }
}
"""


def test_snapshot_keys_ignores_the_insta_header() -> None:
    keys = lvb.snapshot_keys(SNAPSHOT)
    assert keys == frozenset({"schema_version", "result", "width", "height"})
    assert "source" not in keys
    assert "expression" not in keys


def test_snapshot_keys_are_blind_to_a_changed_value() -> None:
    assert lvb.snapshot_keys(SNAPSHOT) == lvb.snapshot_keys(
        SNAPSHOT.replace('"width": 4', '"width": 8')
    )


def test_a_renamed_key_drops_a_key_and_an_added_one_does_not() -> None:
    before = lvb.snapshot_keys(SNAPSHOT)
    renamed = lvb.snapshot_keys(SNAPSHOT.replace('"height"', '"rows"'))
    added = lvb.snapshot_keys(SNAPSHOT.replace('"height": 2', '"height": 2,\n    "rows": 2'))
    assert before - renamed == frozenset({"height"})
    assert not before - added


def test_struct_fields_reads_the_named_block_only() -> None:
    assert lvb.struct_fields(CACHE, "Header") == frozenset({"format_version", "digest", "width"})
    assert lvb.struct_fields(CACHE, "Document") is None


def test_struct_fields_stops_at_the_closing_brace() -> None:
    # `impl Header`'s body sits below the block and names a field per line of its own.
    assert "new" not in (lvb.struct_fields(CACHE, "Header") or frozenset())


def test_constant_value_is_the_literal() -> None:
    assert lvb.constant_value(CACHE, "FORMAT_VERSION") == "1"
    assert lvb.constant_value(CACHE.replace("= 1;", "= 2;"), "FORMAT_VERSION") == "2"
    assert lvb.constant_value(CACHE, "SCHEMA_VERSION") is None


def test_constant_value_ignores_the_comment_above_it() -> None:
    edited = CACHE.replace("Bumped when the header's fields move.", "Bumped when the layout moves.")
    assert lvb.constant_value(edited, "FORMAT_VERSION") == lvb.constant_value(
        CACHE, "FORMAT_VERSION"
    )


def test_the_watched_blocks_are_the_ones_on_disk() -> None:
    # The triggers name blocks by string, so a rename in the crate leaves them naming nothing. The
    # hook fires on that, and this fails before a commit reaches it.
    for path, name, _ in lvb.STRUCT_TRIGGERS:
        text = (lvb.REPO_ROOT / path).read_text(encoding="utf-8")
        assert lvb.struct_fields(text, name), f"{path} has no struct {name}"


def test_the_watched_constants_are_the_ones_on_disk() -> None:
    report = (lvb.REPO_ROOT / lvb.REPORT).read_text(encoding="utf-8")
    assert lvb.constant_value(report, "SCHEMA_VERSION")
    for _, name, versioned in lvb.STRUCT_TRIGGERS:
        assert versioned, f"struct {name} governs no constant"
        for path in versioned:
            text = (lvb.REPO_ROOT / path).read_text(encoding="utf-8")
            assert lvb.constant_value(text, "FORMAT_VERSION"), f"{path} has no FORMAT_VERSION"


def test_the_attestation_is_watched_against_both_format_versions() -> None:
    # ADR 0007 decision 2 flattens one shape into two separately versioned documents, so a field
    # added to it owes both bumps. The pairing is the whole reason a trigger names files rather than
    # reading the constant beside the block, and dropping it would leave the ledger's readers
    # unguarded while the hook still passed.
    governed = {name: files for _, name, files in lvb.STRUCT_TRIGGERS}
    assert set(governed["Attestation"]) == {lvb.TABLE_CACHE, lvb.LEDGER_CACHE}
    assert governed["Header"] == (lvb.TABLE_CACHE,)
    assert governed["Document"] == (lvb.LEDGER_CACHE,)


def test_the_snapshot_directory_holds_snapshots_this_can_read() -> None:
    snapshots = sorted((lvb.REPO_ROOT / lvb.SNAPSHOT_DIR).glob("*.snap"))
    assert snapshots
    for snapshot in snapshots:
        keys = lvb.snapshot_keys(snapshot.read_text(encoding="utf-8"))
        assert "schema_version" in keys, f"{snapshot.name} parsed to {sorted(keys)}"
