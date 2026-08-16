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
    # The document's own earth model, carried down so a caller sizing a cap never needs a radius of
    # its own — `geodesy.rs` owns that number and a second copy in Python is the defect the "Ground
    # distance" invariant names.
    earth_radius_km: float


class Envelope(BaseModel):
    model_config = _CONFIG

    schema_version: Annotated[int, Field(le=SCHEMA_VERSION)]
    document: DocumentKind
    earth_model: EarthModel


class _Payload(BaseModel, ABC):
    model_config = _CONFIG

    @abstractmethod
    def as_circle(self, earth_radius_km: float) -> Circle: ...


class _Measured(_Payload):
    centre: Coordinate
    radius_km: float
    population: float
    share_of_total: float

    def as_circle(self, earth_radius_km: float) -> Circle:
        return Circle(
            centre=self.centre,
            radius_km=self.radius_km,
            population=self.population,
            share=self.share_of_total,
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

    def as_circle(self, earth_radius_km: float) -> Circle:
        return Circle(
            centre=self.circle.centre,
            radius_km=self.circle.radius_km,
            population=self.circle.population,
            share=self.circle.share_achieved,
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
    return payload.model_validate(document.get("result")).as_circle(envelope.earth_model.radius_km)
