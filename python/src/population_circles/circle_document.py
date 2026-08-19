from abc import ABC, abstractmethod
from collections.abc import Mapping
from typing import Annotated, Literal

from pydantic import BaseModel, ConfigDict, Field

# The highest `report::SCHEMA_VERSION` this reader was written against. A document above it may have
# renamed or removed a field, which is the only thing that constant rises for, so it is refused
# rather than read optimistically.
SCHEMA_VERSION = 1

# The nine `report::Document::KIND` strings. Held as a Literal so an unrecognised kind is a schema
# refusal naming `document`, not a KeyError three frames later.
DocumentKind = Literal[
    "distance",
    "grid",
    "table-build",
    "table-query",
    "circle",
    "most-populous",
    "smallest-circle",
    "smallest",
    "sweep",
]

# frozen because nothing downstream of the boundary may edit what the document said, and
# `extra="ignore"` because that is the format's own instruction to consumers (`report.rs` "Growth"):
# a field added additively must not turn into a refusal here.
_CONFIG = ConfigDict(frozen=True, extra="ignore")


class UnsupportedDocumentError(ValueError):
    def __init__(self, kind: str) -> None:
        super().__init__(f"document kind {kind!r} carries no circle to draw")
        self.kind = kind


class MissingDatasetError(ValueError):
    def __init__(self) -> None:
        super().__init__(
            "the document names no dataset under provenance, so there is no attribution for a "
            "figure to carry; rebuild the table with `table build --dataset <name>`",
        )


class MissingTableError(ValueError):
    def __init__(self) -> None:
        super().__init__(
            "the document names no table under provenance, so a figure could not say what answered "
            "it; the digest and the decimation together are what identify one",
        )


class Coordinate(BaseModel):
    model_config = _CONFIG

    lat: float
    lon: float


class EarthModel(BaseModel):
    model_config = _CONFIG

    model: Literal["sphere"]
    radius_km: float


class Circle(BaseModel):
    model_config = _CONFIG

    centre: Coordinate
    radius_km: float
    population: float
    share: float
    # The registry key the document was answered from, carried down for the same reason as the
    # radius below: what a figure credits is a property of the answer, not of the renderer.
    dataset: str
    # The table that answered it, as the document states it. The digest is the raster's and is the
    # same for every table built from one, so the decimation is what tells two of them apart — a
    # caption naming only the digest would read identically for a 30 arc-second and a 5 arc-minute
    # answer.
    table: str
    # The document's own earth model, carried down so a caller sizing a cap never needs a radius of
    # its own — `geodesy.rs` owns that number and a second copy in Python is the defect the "Ground
    # distance" invariant names.
    earth_radius_km: float


class Provenance(BaseModel):
    model_config = _CONFIG

    # Optional in the format, per `report.rs` "Growth", and required by this reader: a figure whose
    # credit cannot be read off its own document would be credited from whatever the renderer was
    # written against, so a document that cannot say is refused rather than drawn.
    dataset: str | None = None
    # Both optional in the format for `dataset`'s reason, and both required by this reader together:
    # the caption a figure owes names the table, and neither half names one alone.
    digest: str | None = None
    decimation: int | None = None


class Envelope(BaseModel):
    model_config = _CONFIG

    schema_version: Annotated[int, Field(le=SCHEMA_VERSION)]
    document: DocumentKind
    earth_model: EarthModel
    # Absent from a document whose command read no cached table, which is every kind this reader
    # refuses anyway.
    provenance: Provenance | None = None


class _Payload(BaseModel, ABC):
    model_config = _CONFIG

    @abstractmethod
    def as_circle(self, earth_radius_km: float, dataset: str, table: str) -> Circle: ...


class _Measured(_Payload):
    centre: Coordinate
    radius_km: float
    population: float
    share_of_total: float

    def as_circle(self, earth_radius_km: float, dataset: str, table: str) -> Circle:
        return Circle(
            centre=self.centre,
            radius_km=self.radius_km,
            population=self.population,
            share=self.share_of_total,
            dataset=dataset,
            table=table,
            earth_radius_km=earth_radius_km,
        )


class _Smallest(BaseModel):
    model_config = _CONFIG

    centre: Coordinate
    radius_km: float
    population: float
    share_achieved: float


class _Searched(_Payload):
    circle: _Smallest

    def as_circle(self, earth_radius_km: float, dataset: str, table: str) -> Circle:
        return Circle(
            centre=self.circle.centre,
            radius_km=self.circle.radius_km,
            population=self.circle.population,
            share=self.circle.share_achieved,
            dataset=dataset,
            table=table,
            earth_radius_km=earth_radius_km,
        )


# The two payload shapes a circle arrives in, and which kinds use which. `smallest` nests its circle
# under a ledger that belongs to the run rather than to any one circle, which is why it is not the
# same shape as the other two rather than a superset of them.
_PAYLOADS: Mapping[str, type[_Payload]] = {
    "circle": _Measured,
    "most-populous": _Measured,
    "smallest": _Searched,
}

CIRCLE_KINDS = frozenset(_PAYLOADS)


def circle_of(document: Mapping[str, object]) -> Circle:
    envelope = Envelope.model_validate(document)
    payload = _PAYLOADS.get(envelope.document)
    if payload is None:
        raise UnsupportedDocumentError(envelope.document)
    provenance = envelope.provenance
    dataset = provenance.dataset if provenance else None
    if dataset is None:
        raise MissingDatasetError
    if provenance is None or provenance.digest is None or provenance.decimation is None:
        raise MissingTableError
    return payload.model_validate(document.get("result")).as_circle(
        envelope.earth_model.radius_km,
        dataset,
        f"{provenance.digest} at decimation {provenance.decimation}",
    )
