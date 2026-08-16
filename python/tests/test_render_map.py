import json
import re
import socket
from pathlib import Path
from typing import Any, NoReturn

import pytest

from population_circles.circle_document import circle_of
from population_circles.circle_geometry import HALF_TURN_DEG, POLE_LAT
from population_circles.dataset_registry import POPULATION_KEY, load
from population_circles.map_frame import ORTHOGRAPHIC, PLATE_CARREE
from population_circles.render_map import COASTLINE, basemap, citation, main, render

REGISTRY = load()
PROSE = Path(__file__).resolve().parents[2] / "data" / "README.md"

CENTRE_LAT = 25.125
CENTRE_LON = 79.708
RADIUS_KM = 1000.0

PNG_MAGIC = b"\x89PNG\r\n\x1a\n"
NO_NETWORK = "a figure is drawn from the committed basemap and reaches no network"


def document() -> dict[str, Any]:
    return {
        "schema_version": 1,
        "document": "most-populous",
        "tool": "popcircles",
        "tool_version": "0.1.0",
        "earth_model": {"model": "sphere", "radius_km": 6371.0088},
        "result": {
            "centre": {"lat": CENTRE_LAT, "lon": CENTRE_LON},
            "row": 508,
            "col": 3116,
            "radius_km": RADIUS_KM,
            "population": 1254363867.93,
            "total_population": 7757982599.323671,
            "share_of_total": 0.1616868627727305,
        },
    }


def normalised(text: str) -> str:
    # Blockquote markers, emphasis and autolink brackets go, then whitespace collapses. The registry
    # writes the citation as Markdown and a figure draws it as prose, so comparing them at all means
    # discounting the markup that only one of the two carries.
    return " ".join(re.sub(r"[>*<]", " ", text).split())


def test_the_citation_is_the_text_the_registry_owns() -> None:
    # data/registry.toml owns the wording now, and PROSE restates it for a human. Both halves are
    # checked, so a drift fails here rather than shipping a figure that credits nobody.
    assert normalised(citation()) == normalised(REGISTRY.datasets[POPULATION_KEY].attribution)
    assert normalised(citation()) in normalised(PROSE.read_text(encoding="utf-8"))


def test_the_basemap_is_the_committed_one() -> None:
    # The registry's row is what a reader is sent to; this is the half a test can hold — the file is
    # in the tree, outside LFS, and parses into coastlines spanning the world.
    assert COASTLINE.is_file()
    assert not COASTLINE.read_bytes().startswith(b"version https://git-lfs")
    coastline = basemap(COASTLINE)
    west, south, east, north = coastline.bounds
    assert (west, east) == pytest.approx((-HALF_TURN_DEG, HALF_TURN_DEG), abs=1e-6)
    assert -POLE_LAT < south < north < POLE_LAT


def test_the_footer_artist_carries_the_citation() -> None:
    figure = render(circle_of(document()), PLATE_CARREE, coastlines=False)
    drawn_text = " ".join(normalised(artist.get_text()) for artist in figure.texts)
    assert normalised(citation()) in drawn_text


def test_the_title_states_the_radius_the_share_and_the_centre() -> None:
    figure = render(circle_of(document()), ORTHOGRAPHIC, coastlines=False)
    drawn_text = " ".join(normalised(artist.get_text()) for artist in figure.texts)
    assert "1,000 km circle" in drawn_text
    assert "16.17% of the population" in drawn_text
    assert "1,254,363,868 people" in drawn_text


@pytest.mark.parametrize("projection", [PLATE_CARREE, ORTHOGRAPHIC])
def test_a_complete_figure_is_written_where_it_is_told(projection: str, tmp_path: Path) -> None:
    # The figure a test writes goes to tmp_path: no rendered map enters the repository, which is the
    # first project invariant and the reason there is no baseline image to compare against.
    source = tmp_path / "circle.json"
    source.write_text(json.dumps(document()), encoding="utf-8")
    output = tmp_path / "figures" / "map.png"

    argv = ["--input", str(source), "--output", str(output), "--projection", projection]
    assert main(argv) == 0
    assert output.read_bytes().startswith(PNG_MAGIC)


def test_a_complete_figure_needs_no_network(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    # The claim the committed basemap exists to make, proved by denial rather than by reading the
    # imports: sockets are taken away for the duration, so a figure that still needs a download
    # fails here instead of passing on whichever machine happened to have one.
    def refuse(*_args: object, **_kwargs: object) -> NoReturn:
        raise AssertionError(NO_NETWORK)

    for name in ("socket", "create_connection", "socketpair"):
        monkeypatch.setattr(socket, name, refuse)

    source = tmp_path / "circle.json"
    source.write_text(json.dumps(document()), encoding="utf-8")
    output = tmp_path / "map.png"

    assert main(["--input", str(source), "--output", str(output)]) == 0
    assert output.read_bytes().startswith(PNG_MAGIC)
