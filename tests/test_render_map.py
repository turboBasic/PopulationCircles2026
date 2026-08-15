import json
import re
from pathlib import Path
from typing import Any

import pytest
from circle_document import circle_of
from render_map import CITATION, ORTHOGRAPHIC, PLATE_CARREE, main, render

REGISTRY = Path(__file__).resolve().parent.parent / "data" / "README.md"

CENTRE_LAT = 25.125
CENTRE_LON = 79.708
RADIUS_KM = 1000.0


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
    # data/README.md "Licence and attribution" owns the wording. Checked rather than trusted, so a
    # drift between the two fails here instead of shipping a figure that credits nobody.
    assert normalised(CITATION) in normalised(REGISTRY.read_text(encoding="utf-8"))


def test_the_footer_artist_carries_the_citation() -> None:
    # Without coastlines, so nothing here reaches the network.
    figure = render(circle_of(document()), PLATE_CARREE, coastlines=False)
    drawn_text = " ".join(normalised(artist.get_text()) for artist in figure.texts)
    assert normalised(CITATION) in drawn_text


def test_the_title_states_the_radius_the_share_and_the_centre() -> None:
    figure = render(circle_of(document()), ORTHOGRAPHIC, coastlines=False)
    drawn_text = " ".join(normalised(artist.get_text()) for artist in figure.texts)
    assert "1,000 km circle" in drawn_text
    assert "16.17% of the population" in drawn_text
    assert "1,254,363,868 people" in drawn_text


def test_a_figure_is_written_where_it_is_told(tmp_path: Path) -> None:
    # The figure a test writes goes to tmp_path: no rendered map enters the repository, which is the
    # first project invariant and the reason there is no baseline image to compare against.
    source = tmp_path / "circle.json"
    source.write_text(json.dumps(document()), encoding="utf-8")
    output = tmp_path / "figures" / "map.png"

    assert main(["--input", str(source), "--output", str(output), "--no-coastlines"]) == 0
    assert output.read_bytes().startswith(b"\x89PNG\r\n\x1a\n")


@pytest.mark.network
def test_a_full_figure_renders_with_coastlines(tmp_path: Path) -> None:
    # The only test that draws the complete figure, and the reason it is marked: `coastlines()`
    # fetches Natural Earth from naturalearth.s3.amazonaws.com on first use.
    source = tmp_path / "circle.json"
    source.write_text(json.dumps(document()), encoding="utf-8")
    output = tmp_path / "map.png"

    assert main(["--input", str(source), "--output", str(output)]) == 0
    assert output.read_bytes().startswith(b"\x89PNG\r\n\x1a\n")
