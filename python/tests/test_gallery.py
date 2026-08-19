import json
from pathlib import Path
from typing import Any

import pytest

from population_circles.circle_document import CIRCLE_KINDS
from population_circles.gallery import build
from population_circles.render_map import UnregisteredDatasetError

CORPUS = Path(__file__).resolve().parents[2] / "results"
DATASET = "population-count-2020-30arcsec"
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"


def document(kind: str, dataset: str = DATASET) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "document": kind,
        "tool": "popcircles",
        "tool_version": "0.1.0",
        "earth_model": {"model": "sphere", "radius_km": 6371.0088},
        "provenance": {"digest": "0xf17aa802a6890f0c", "decimation": 10, "dataset": dataset},
        "result": {
            "centre": {"lat": 25.125, "lon": 79.708},
            "radius_km": 1000.0,
            "population": 1254363867.93,
            "share_of_total": 0.1616868627727305,
            # A sweep's own payload, carried so the skipped fixture is a document this corpus could
            # actually hold rather than a renderable one relabelled.
            "circles": [],
        },
    }


def written(directory: Path, name: str, payload: dict[str, Any]) -> None:
    (directory / name).write_text(json.dumps(payload), encoding="utf-8")


def test_a_renderable_document_is_drawn_and_an_unrenderable_one_is_named(tmp_path: Path) -> None:
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    written(corpus, "drawable.json", document("most-populous"))
    written(corpus, "not-drawable.json", document("sweep"))

    drawn, skipped = build(corpus, tmp_path / "gallery")

    assert [one.figure.name for one in drawn] == ["drawable.png"]
    assert drawn[0].figure.read_bytes().startswith(PNG_MAGIC)
    assert [(one.document.name, one.kind) for one in skipped] == [("not-drawable.json", "sweep")]
    # The kind is named on the page too, since a reader of the gallery has no access to this list.
    assert "sweep" in (tmp_path / "gallery" / "index.html").read_text(encoding="utf-8")


def test_a_document_that_fails_to_draw_publishes_no_gallery_at_all(tmp_path: Path) -> None:
    # The box this pins is "fails the build rather than publishing a gallery with a hole in it", so
    # what it asserts is the absence of the page: a collected failure would have written one.
    corpus = tmp_path / "corpus"
    corpus.mkdir()
    written(corpus, "drawable.json", document("most-populous"))
    written(corpus, "unregistered.json", document("most-populous", "no-such-dataset"))
    output = tmp_path / "gallery"

    with pytest.raises(UnregisteredDatasetError) as caught:
        build(corpus, output)

    assert "unregistered.json" in " ".join(caught.value.__notes__)
    assert not (output / "index.html").exists()


def test_the_committed_corpus_draws_every_kind_this_reader_holds(tmp_path: Path) -> None:
    # The gallery CI publishes, built here so a broken render fails a pull request. Which documents
    # are drawn is read off the corpus rather than listed: a kind added to the reader, or a document
    # added to `results/`, moves this assertion without editing it.
    output = tmp_path / "gallery"
    drawn, skipped = build(CORPUS, output)

    committed = sorted(CORPUS.glob("*.json"))
    assert len(drawn) + len(skipped) == len(committed)
    assert drawn, "the committed corpus holds no document this gallery can draw"
    for one in drawn:
        assert one.figure.read_bytes().startswith(PNG_MAGIC)
        assert (output / one.document.name).is_file()
    for one in skipped:
        assert one.kind not in CIRCLE_KINDS
