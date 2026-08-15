import math

import cartopy.crs as ccrs
import pytest
from circle_document import Coordinate
from circle_geometry import METRES_PER_KM, QUAD_SEGS, Cap, boundary, cap, drawn
from pyproj import Geod
from shapely.geometry import MultiPolygon

EARTH_RADIUS_KM = 6371.0088

# The four cases the drawing primitive was chosen on: a cap crossing the seam, one over each pole,
# and one crossing nothing. The last is the only one that round-trips, which is why it is here.
SEAM = Coordinate(lat=10.0, lon=178.0)
NORTH = Coordinate(lat=78.0, lon=20.0)
SOUTH = Coordinate(lat=-80.0, lon=-170.0)
CLEAN = Coordinate(lat=30.0, lon=100.0)

SEAM_RADIUS_KM = 3000.0
NORTH_RADIUS_KM = 3000.0
SOUTH_RADIUS_KM = 4000.0
CLEAN_RADIUS_KM = 3300.0

CASES = (
    (SEAM, SEAM_RADIUS_KM),
    (NORTH, NORTH_RADIUS_KM),
    (SOUTH, SOUTH_RADIUS_KM),
    (CLEAN, CLEAN_RADIUS_KM),
)
CASE_IDS = ("antimeridian", "north-pole", "south-pole", "no-crossing")

# shapely starts a buffer's ring due east and walks it clockwise, so a quarter of QUAD_SEGS * 4
# sides is a quarter turn. Each index below is paired with the azimuth that vertex lies on.
CARDINALS = ((0, 90.0), (QUAD_SEGS, 180.0), (2 * QUAD_SEGS, -90.0), (3 * QUAD_SEGS, 0.0))

# A ring of QUAD_SEGS * 4 sides is that many vertices plus the repeated close.
RING_VERTICES = 4 * QUAD_SEGS + 1

# Measured, and the figures the two-object split exists for: closing a cap over a pole synthesises
# four vertices, two of them sitting exactly on the pole, and cutting one down the ±180 meridian
# synthesises seven. Neither lies on the cap's boundary.
POLE_CLOSED_VERTICES = 725
SEAM_CUT_VERTICES = 728
POLE_LAT = 90.0
SEAM_LON = 180.0
VERTICES_ON_THE_POLE = 2
SEAM_PARTS = 2


def great_circle_km(a: Coordinate, b: Coordinate) -> float:
    # Written here rather than imported: a vertex checked against the same code that placed it is
    # not checked. This is the haversine on the sphere `geodesy.rs` owns, which is the model the
    # document under test published.
    phi_a, phi_b = math.radians(a.lat), math.radians(b.lat)
    half_lat = math.sin((phi_b - phi_a) / 2.0) ** 2
    half_lon = math.sin(math.radians(b.lon - a.lon) / 2.0) ** 2
    chord = half_lat + math.cos(phi_a) * math.cos(phi_b) * half_lon
    return 2.0 * EARTH_RADIUS_KM * math.asin(math.sqrt(chord))


def built(centre: Coordinate, radius_km: float) -> Cap:
    return cap(centre, radius_km, EARTH_RADIUS_KM)


def parts(shape: MultiPolygon) -> list[tuple[float, float]]:
    return [(x, y) for polygon in shape.geoms for x, y in polygon.exterior.coords]


def projected(shape: Cap) -> MultiPolygon:
    # The target is built on the cap's own globe: another one makes PROJ shift the datum, which is a
    # second earth model arriving by default. The isinstance is the topological claim itself —
    # `drawn` is typed as any geometry because a cap at the seam genuinely becomes two polygons.
    out = drawn(shape, ccrs.PlateCarree(globe=shape.globe))
    assert isinstance(out, MultiPolygon)
    return out


@pytest.mark.parametrize(("centre", "radius_km"), CASES, ids=CASE_IDS)
def test_every_boundary_vertex_is_the_radius_from_the_centre(
    centre: Coordinate,
    radius_km: float,
) -> None:
    ring = boundary(built(centre, radius_km))
    assert len(ring) == RING_VERTICES
    for vertex in ring:
        assert great_circle_km(centre, vertex) == pytest.approx(radius_km, abs=1e-6)


def test_the_cardinal_vertices_match_the_direct_geodesic_problem() -> None:
    # pyproj's direct geodesic problem at flattening zero: a different library than the one that
    # drew the ring, answering about the same sphere.
    geod = Geod(a=EARTH_RADIUS_KM * METRES_PER_KM, f=0.0)
    ring = boundary(built(CLEAN, CLEAN_RADIUS_KM))
    for index, azimuth in CARDINALS:
        lon, lat, _ = geod.fwd(CLEAN.lon, CLEAN.lat, azimuth, CLEAN_RADIUS_KM * METRES_PER_KM)
        assert ring[index].lat == pytest.approx(lat, abs=1e-9)
        assert ring[index].lon == pytest.approx(lon, abs=1e-9)


def test_a_cap_crossing_the_antimeridian_is_drawn_in_two_parts() -> None:
    # The case `ax.fill(..., transform=ccrs.Geodetic())` fills the complement of. Two parts and a
    # longitude span reaching both limits is what a cut down the seam looks like.
    shape = projected(built(SEAM, SEAM_RADIUS_KM))
    assert len(shape.geoms) == SEAM_PARTS
    west, _, east, _ = shape.bounds
    assert (west, east) == (-SEAM_LON, SEAM_LON)
    assert len(parts(shape)) == SEAM_CUT_VERTICES


def test_a_cap_over_the_north_pole_is_closed_at_the_pole() -> None:
    # The vertices the closing synthesises are the point of the two-object split: the ring is 721
    # real points every one 3000 km out, and the drawn polygon is 725 including two at the pole,
    # which is 1334.341 km from this centre and therefore on no boundary.
    cap_over_pole = built(NORTH, NORTH_RADIUS_KM)
    ring = boundary(cap_over_pole)
    assert len(ring) == RING_VERTICES
    assert {round(great_circle_km(NORTH, vertex), 6) for vertex in ring} == {NORTH_RADIUS_KM}

    shape = projected(cap_over_pole)
    assert len(shape.geoms) == 1
    vertices = parts(shape)
    assert len(vertices) == POLE_CLOSED_VERTICES
    assert sum(1 for _, lat in vertices if lat == POLE_LAT) == VERTICES_ON_THE_POLE
    assert shape.bounds == pytest.approx((-SEAM_LON, 51.020, SEAM_LON, POLE_LAT), abs=1e-3)


def test_a_cap_over_the_south_pole_is_closed_at_the_pole() -> None:
    shape = projected(built(SOUTH, SOUTH_RADIUS_KM))
    vertices = parts(shape)
    assert len(vertices) == POLE_CLOSED_VERTICES
    assert sum(1 for _, lat in vertices if lat == -POLE_LAT) == VERTICES_ON_THE_POLE
    assert shape.bounds == pytest.approx((-SEAM_LON, -POLE_LAT, SEAM_LON, -44.027), abs=1e-3)


def test_a_cap_crossing_nothing_synthesises_no_vertex() -> None:
    # The only case where one assertion would have done for both objects, and the reason it cannot
    # be the only case tested.
    shape = projected(built(CLEAN, CLEAN_RADIUS_KM))
    assert len(shape.geoms) == 1
    assert len(parts(shape)) == RING_VERTICES
    assert shape.bounds == pytest.approx((65.130, 0.322, 134.870, 59.678), abs=1e-3)
    for lon, lat in parts(shape):
        assert great_circle_km(CLEAN, Coordinate(lat=lat, lon=lon)) == pytest.approx(
            CLEAN_RADIUS_KM,
            abs=1e-6,
        )


@pytest.mark.parametrize(("centre", "radius_km"), CASES, ids=CASE_IDS)
def test_no_drawn_vertex_lies_outside_the_cap(centre: Coordinate, radius_km: float) -> None:
    # The one distance claim that survives synthesised vertices. Asserting each is *at* the radius
    # would fail on precisely the two cases this primitive exists to get right.
    for lon, lat in parts(projected(built(centre, radius_km))):
        assert great_circle_km(centre, Coordinate(lat=lat, lon=lon)) <= radius_km + 1e-6
