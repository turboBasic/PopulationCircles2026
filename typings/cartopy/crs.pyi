import pyproj
from shapely.geometry.base import BaseGeometry

class Globe:
    def __init__(
        self,
        *,
        ellipse: str | None = ...,
        semimajor_axis: float | None = ...,
        semiminor_axis: float | None = ...,
        flattening: float | None = ...,
    ) -> None: ...

# Declared as pyproj's CRS because that is what it is, which is what lets `Transformer.from_crs`
# take one of these directly rather than through a PROJ string this module would have to build a
# second time.
class CRS(pyproj.CRS):
    def as_geodetic(self) -> CRS: ...

class Projection(CRS):
    # Widening to BaseGeometry rather than naming Polygon: PROJ splits a cap at the seam, so a
    # polygon in goes to a MultiPolygon out and the two hard cases are exactly the ones a narrower
    # return type would hide behind a cast.
    def project_geometry(self, geometry: BaseGeometry, src_crs: Projection) -> BaseGeometry: ...

class PlateCarree(Projection):
    def __init__(self, central_longitude: float = ..., globe: Globe | None = ...) -> None: ...

class Orthographic(Projection):
    def __init__(
        self,
        central_longitude: float = ...,
        central_latitude: float = ...,
        globe: Globe | None = ...,
    ) -> None: ...

class AzimuthalEquidistant(Projection):
    def __init__(
        self,
        central_longitude: float = ...,
        central_latitude: float = ...,
        globe: Globe | None = ...,
    ) -> None: ...
