import hashlib
import struct
from pathlib import PurePosixPath

import pytest
from pydantic import ValidationError

from population_circles.dataset_registry import (
    POPULATION_KEY,
    REGISTRY,
    BoundaryVector,
    PopulationRaster,
    load,
    parse,
)

# The literal `crates/popcircles/tests/registry_raster.rs` builds its `RasterSpec` from. The
# registry spells the same sentinel with more digits, and the claim is that both land on one f32 —
# so this file keeps the two halves from drifting, since no gate compiles that ignored test.
RUST_NODATA = -3.402_823e38

RASTER_SPEC = "crates/popcircles/tests/registry_raster.rs"


def repo_root() -> PurePosixPath:
    return PurePosixPath(REGISTRY.parent.parent.as_posix())


def test_every_key_is_its_paths_filename_stem() -> None:
    # The rule that makes a key, a file, a release asset and a heading in data/README.md one string.
    for key, dataset in load().datasets.items():
        assert PurePosixPath(dataset.path).stem == key


def test_a_key_that_is_not_its_stem_is_refused() -> None:
    # The negative half: the check fires at load, not when data:get happens to look.
    text = REGISTRY.read_text(encoding="utf-8").replace(
        f"[datasets.{POPULATION_KEY}]",
        "[datasets.population-count-2020]",
    )
    with pytest.raises(ValidationError, match="whose stem is"):
        parse(text)


def test_the_nodata_literal_round_trips_to_the_rust_constant() -> None:
    # Compared bit for bit by the reader, so "close enough" is not a property this value may have.
    registry_nodata = load().datasets[POPULATION_KEY]
    assert isinstance(registry_nodata, PopulationRaster)
    assert struct.pack("<f", registry_nodata.nodata) == struct.pack("<f", RUST_NODATA)


def test_the_rust_spec_still_carries_the_constant_this_pins() -> None:
    # Without this, retuning the Rust literal would leave the test above passing against a constant
    # nothing reads. The ignored test it lives in is outside every gate.
    source = (REGISTRY.parent.parent / RASTER_SPEC).read_text(encoding="utf-8")
    assert "-3.402_823e38" in source


def test_each_row_describes_the_file_on_disk() -> None:
    # bytes and sha256 are what a fetch enforces and a reader verifies, so a stale one is worse than
    # none — measured here rather than trusted.
    for dataset in load().datasets.values():
        content = dataset.file.read_bytes()
        assert len(content) == dataset.bytes
        assert hashlib.sha256(content).hexdigest() == dataset.sha256


def test_the_committed_dataset_has_no_fetch_url_and_the_published_one_does() -> None:
    datasets = load().datasets
    published = datasets[POPULATION_KEY]
    assert published.fetch_url is not None
    assert published.fetch_url.endswith(PurePosixPath(published.path).name)
    committed = [d for d in datasets.values() if isinstance(d, BoundaryVector)]
    assert committed
    for dataset in committed:
        assert dataset.fetch_url is None


def test_an_attribution_is_present_even_when_nothing_is_owed() -> None:
    # Empty rather than absent is the whole point: a consumer reads one field for every dataset.
    attributions = {key: d.attribution for key, d in load().datasets.items()}
    assert attributions["coastline-1to110m"] == ""
    assert "CIESIN" in attributions[POPULATION_KEY]


def test_an_unknown_kind_is_refused_rather_than_read_as_a_bare_dataset() -> None:
    text = REGISTRY.read_text(encoding="utf-8").replace(
        'kind = "boundary-vector"',
        'kind = "coastline"',
    )
    with pytest.raises(ValidationError):
        parse(text)
