from dataclasses import dataclass

import cartopy.crs as ccrs
from circle_document import Coordinate
from pyproj import Transformer
from shapely.geometry import Point
from shapely.geometry.base import BaseGeometry
from shapely.geometry.polygon import Polygon

METRES_PER_KM = 1000.0

# Per quadrant, so the ring shapely returns has 720 sides. The chord then cuts R(1 - cos(pi/720))
# inside the arc, which at 3300 km is 31.41 m — a thirtieth of the registry raster's 926.6 m cell at
# the equator, so the shortfall is below anything a figure over that raster can show. Raising it
# costs vertices in every drawn polygon and buys nothing visible.
QUAD_SEGS = 180


@dataclass(frozen=True)
class Cap:
    # A circle about the origin in the source CRS's metres, which is what a spherical cap *is* in an
    # azimuthal equidistant projection centred on it. Every hard case — the antimeridian, either
    # pole — is ordinary here and becomes PROJ's problem rather than a traversal's.
    polygon: Polygon
    source: ccrs.AzimuthalEquidistant
    # Published so a caller drawing this cap builds its target projection on the same sphere. A
    # target on another one makes PROJ shift the datum, which is a second earth model arriving by
    # default rather than by decision.
    globe: ccrs.Globe
    centre: Coordinate
    radius_km: float


def cap(centre: Coordinate, radius_km: float, earth_radius_km: float) -> Cap:
    metres = earth_radius_km * METRES_PER_KM
    globe = ccrs.Globe(ellipse=None, semimajor_axis=metres, semiminor_axis=metres)
    source = ccrs.AzimuthalEquidistant(
        central_longitude=centre.lon,
        central_latitude=centre.lat,
        globe=globe,
    )
    return Cap(
        polygon=Point(0.0, 0.0).buffer(radius_km * METRES_PER_KM, quad_segs=QUAD_SEGS),
        source=source,
        globe=globe,
        centre=centre,
        radius_km=radius_km,
    )


def boundary(built: Cap) -> tuple[Coordinate, ...]:
    # The buffer's own vertices, one transform each. Every one is a real point on the cap's boundary
    # and `radius_km` from the centre, which is the claim a test can make here and cannot make about
    # `drawn`: closing a projected cap synthesises vertices that lie on no boundary at all.
    to_geodetic = Transformer.from_crs(built.source, built.source.as_geodetic(), always_xy=True)
    return tuple(
        Coordinate(lat=lat, lon=lon)
        for lon, lat in (to_geodetic.transform(x, y) for x, y in built.polygon.exterior.coords)
    )


def drawn(built: Cap, target: ccrs.Projection) -> BaseGeometry:
    # One polygon in, one or two out: PROJ cuts the cap at the seam and closes it over a pole, which
    # is the whole reason a ring of coordinates is not the drawing path. Build `target` from
    # `built.globe`.
    return target.project_geometry(built.polygon, built.source)
