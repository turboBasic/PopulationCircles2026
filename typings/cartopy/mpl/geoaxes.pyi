from collections.abc import Iterable

from cartopy.crs import Projection
from matplotlib.artist import Artist
from matplotlib.axes import Axes
from shapely.geometry.base import BaseGeometry

# GeoAxes really does subclass Axes, so titles, markers and the rest arrive from matplotlib's own
# annotations and only cartopy's four additions are declared here. It is a type at all because
# `Figure.add_subplot(projection=...)` is annotated as returning `Axes3D`, which makes every call
# below an attribute error without this file.
class GeoAxes(Axes):
    projection: Projection
    def set_global(self) -> None: ...
    def coastlines(self, resolution: str = ..., color: str = ...) -> Artist: ...
    def gridlines(self, *, draw_labels: bool = ..., linewidth: float = ...) -> Artist: ...
    def add_geometries(
        self,
        geoms: Iterable[BaseGeometry],
        crs: Projection,
        *,
        facecolor: str = ...,
        edgecolor: str = ...,
        alpha: float = ...,
        linewidth: float = ...,
        zorder: float = ...,
    ) -> Artist: ...
