from typing import Any

import pytest
from circle_document import (
    CIRCLE_KINDS,
    SCHEMA_VERSION,
    UnsupportedDocumentError,
    circle_of,
)
from pydantic import ValidationError

# Every fixture below is a dictionary, never a file: the renderer's input is one JSON path and its
# tests open nothing at all, which is what keeps them runnable on a clone with no LFS content. The
# figures are `report.rs`'s own snapshots, named so a change to one moves every assertion over it.
EARTH_RADIUS_KM = 6371.0088
EARTH_MODEL = {"model": "sphere", "radius_km": EARTH_RADIUS_KM}

MEASURED_LAT = 45.0
MEASURED_LON = 15.0
MEASURED_RADIUS_KM = 1200.0
MEASURED_POPULATION = 820.0
MEASURED_SHARE = 0.0038996366679982498

SEARCHED_LAT = -85.0
SEARCHED_RADIUS_KM = 2639
SEARCHED_SHARE = 0.25031387319522913


def envelope(kind: str, result: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "document": kind,
        "tool": "popcircles",
        "tool_version": "0.1.0",
        "earth_model": EARTH_MODEL,
        "result": result,
    }


def measured() -> dict[str, Any]:
    return {
        "requested": {"lat": 48.0, "lon": 11.0},
        "centre": {"lat": MEASURED_LAT, "lon": MEASURED_LON},
        "row": 4,
        "col": 19,
        "radius_km": MEASURED_RADIUS_KM,
        "population": MEASURED_POPULATION,
        "total_population": 210276.0,
        "share_of_total": MEASURED_SHARE,
    }


def searched() -> dict[str, Any]:
    return {
        "ledger": {"path": "out/radii.json", "radii": 24},
        "circle": {
            "radius_km": SEARCHED_RADIUS_KM,
            "centre": {"lat": SEARCHED_LAT, "lon": 105.0},
            "population": 52635.0,
            "share_achieved": SEARCHED_SHARE,
            "covers_whole_grid": False,
        },
    }


def locations(error: ValidationError) -> list[tuple[int | str, ...]]:
    return [detail["loc"] for detail in error.errors()]


def test_a_named_circle_reads_as_a_circle() -> None:
    circle = circle_of(envelope("circle", measured()))
    assert circle.centre.lat == MEASURED_LAT
    assert circle.centre.lon == MEASURED_LON
    assert circle.radius_km == MEASURED_RADIUS_KM
    assert circle.population == MEASURED_POPULATION
    assert circle.earth_radius_km == EARTH_RADIUS_KM


def test_the_most_populous_circle_reads_as_a_circle() -> None:
    circle = circle_of(envelope("most-populous", measured()))
    assert circle.radius_km == MEASURED_RADIUS_KM
    assert circle.share == MEASURED_SHARE


def test_a_smallest_document_reads_the_circle_under_its_ledger() -> None:
    # The share arrives under a different key here — `share_achieved`, not `share_of_total` — which
    # is why this kind is a payload shape of its own rather than a superset of the other two.
    circle = circle_of(envelope("smallest", searched()))
    assert circle.radius_km == SEARCHED_RADIUS_KM
    assert circle.share == SEARCHED_SHARE
    assert circle.centre.lat == SEARCHED_LAT


def test_a_later_schema_version_is_refused_on_the_version_field() -> None:
    document = envelope("circle", measured())
    document["schema_version"] = SCHEMA_VERSION + 1
    with pytest.raises(ValidationError) as caught:
        circle_of(document)
    assert locations(caught.value) == [("schema_version",)]


def test_an_unrecognised_kind_is_refused_on_the_document_field() -> None:
    with pytest.raises(ValidationError) as caught:
        circle_of(envelope("valeriepieris", measured()))
    assert locations(caught.value) == [("document",)]


def test_another_earth_model_is_refused_on_the_model_field() -> None:
    document = envelope("circle", measured())
    document["earth_model"] = {"model": "wgs84", "radius_km": 6378.137}
    with pytest.raises(ValidationError) as caught:
        circle_of(document)
    assert locations(caught.value) == [("earth_model", "model")]


def test_a_recognised_kind_carrying_no_circle_names_the_kind() -> None:
    # `distance` is a kind this reader knows and cannot draw, which is a different refusal from the
    # unrecognised one above and the reason it is not folded into that Literal.
    with pytest.raises(UnsupportedDocumentError) as caught:
        circle_of(envelope("distance", {"great_circle_km": 966.3013398709427}))
    assert caught.value.kind == "distance"
    assert "distance" in str(caught.value)


def test_the_three_circle_bearing_kinds_are_the_ones_documented() -> None:
    assert sorted(CIRCLE_KINDS) == ["circle", "most-populous", "smallest"]


def test_a_field_the_reader_does_not_know_is_ignored_rather_than_refused() -> None:
    # `report.rs` "Growth" rules the format additive and tells consumers to ignore what they do not
    # know. A reader that refused instead would break on the next field the format publishes.
    document = envelope("circle", measured())
    document["result"]["a_field_from_a_later_release"] = 1
    document["a_document_level_field_from_a_later_release"] = 2
    assert circle_of(document).radius_km == MEASURED_RADIUS_KM
