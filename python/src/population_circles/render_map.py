import argparse
import json
import textwrap
from collections.abc import Mapping, Sequence
from pathlib import Path

import matplotlib as mpl
from matplotlib import pyplot as plt
from matplotlib.axes import Axes
from matplotlib.collections import LineCollection
from matplotlib.figure import Figure
from matplotlib.patches import PathPatch
from matplotlib.path import Path as DrawPath
from shapely.geometry import shape
from shapely.geometry.base import BaseGeometry
from shapely.geometry.polygon import Polygon
from shapely.ops import unary_union

from population_circles.circle_document import Circle, circle_of
from population_circles.circle_geometry import cap, linestrings, polygons
from population_circles.map_frame import PLATE_CARREE, PROJECTIONS, Frame, frame, graticule, project

# Every figure this writes goes to a file, so an interactive backend is never wanted — and asking
# for one is what fails on a machine with no display, rather than falling back.
mpl.use("Agg")

# The committed basemap. Natural Earth 110m, public domain, outside LFS and small enough to read on
# every render — `data/README.md` is the registry that owns its provenance and terms.
COASTLINE = (
    Path(__file__).resolve().parents[3] / "data" / "boundaries" / "ne-110m-coastline.geojson"
)

# CC BY 4.0 requires attribution of anything published from the raster, and `data/README.md`
# "Licence and attribution" is the owner of this text — a figure carrying a different wording is
# drift, which is what python/tests/test_render_map.py fails on.
CITATION = (
    "Center for International Earth Science Information Network — CIESIN — Columbia University. "
    "2018. Gridded Population of the World, Version 4 (GPWv4): Population Count Adjusted to Match "
    "2015 Revision of UN WPP Country Totals, Revision 11. Palisades, NY: NASA Socioeconomic Data "
    "and Applications Center (SEDAC). https://doi.org/10.7927/H4PN93PB"
)

# Bottom to top: the graticule under the coastlines, the circle over both, its centre over that, and
# the frame's own outline last so no fill reaching the limb draws over it.
GRATICULE_LAYER = 1
COASTLINE_LAYER = 2
CIRCLE_LAYER = 3
CENTRE_LAYER = 4
HORIZON_LAYER = 5


def basemap(path: Path) -> BaseGeometry:
    features: Sequence[Mapping[str, dict[str, object]]] = json.loads(
        path.read_text(encoding="utf-8"),
    )["features"]
    return unary_union([shape(feature["geometry"]) for feature in features])


def compound(polygon: Polygon) -> DrawPath:
    # Exterior first and every hole after it, in one path: a cap holding both poles covers the world
    # bar one region, and that region is a hole rather than a second part.
    return DrawPath.make_compound_path(
        DrawPath(list(polygon.exterior.coords)),
        *(DrawPath(list(ring.coords)) for ring in polygon.interiors),
    )


def fill_circle(axes: Axes, geometry: BaseGeometry) -> None:
    for polygon in polygons(geometry):
        axes.add_patch(
            PathPatch(
                compound(polygon),
                facecolor="crimson",
                edgecolor="darkred",
                alpha=0.45,
                linewidth=0.8,
                zorder=CIRCLE_LAYER,
            ),
        )


def outline(axes: Axes, geometry: BaseGeometry) -> None:
    for polygon in polygons(geometry):
        axes.add_patch(
            PathPatch(
                compound(polygon),
                facecolor="none",
                edgecolor="0.4",
                linewidth=0.6,
                zorder=HORIZON_LAYER,
            ),
        )


def stroke(axes: Axes, geometry: BaseGeometry, colour: str, width: float, layer: float) -> None:
    axes.add_collection(
        LineCollection(
            [list(line.coords) for line in linestrings(geometry)],
            colors=colour,
            linewidths=width,
            zorder=layer,
        ),
    )


def title_of(circle: Circle) -> str:
    return (
        f"{circle.radius_km:,.0f} km circle holding {circle.share:.2%} of the population\n"
        f"{circle.population:,.0f} people, centred {circle.centre.lat:.4f}, {circle.centre.lon:.4f}"
    )


def annotate(figure: Figure, y: float, text: str, size: float) -> None:
    # matplotlib types Figure.text's keyword arguments as Unknown, so the ignore lives here once
    # rather than at each of the two callers.
    figure.text(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
        0.5,
        y,
        text,
        horizontalalignment="center",
        fontsize=size,
    )


def draw(axes: Axes, view: Frame, coastline: BaseGeometry | None) -> None:
    stroke(axes, project(view, graticule()), "0.75", 0.3, GRATICULE_LAYER)
    if coastline is not None:
        stroke(axes, project(view, coastline), "dimgrey", 0.5, COASTLINE_LAYER)
    fill_circle(axes, view.circle)
    outline(axes, view.horizon)


def render(circle: Circle, projection: str, *, coastlines: bool) -> Figure:
    built = cap(circle.centre, circle.radius_km, circle.earth_radius_km)
    view = frame(projection, built)
    centre = view.to_frame.transform(circle.centre.lon, circle.centre.lat)

    figure = plt.figure(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
        figsize=(11.0, 6.0),
    )
    # Room for the title above and the wrapped citation below, since both are placed at figure
    # coordinates rather than left to a layout engine.
    figure.subplots_adjust(left=0.03, right=0.97, top=0.87, bottom=0.16)
    axes = figure.add_subplot(1, 1, 1)
    # Equal, because a degree of longitude and a degree of latitude are the same length on the one
    # frame stated in degrees, and the other frame is metres in both directions.
    axes.set_aspect("equal")
    # The horizon is drawn rather than left to the spines: one frame's outline is a rectangle and
    # the other's is a disc, and only one of those is an axes frame.
    axes.set_axis_off()
    west, south, east, north = view.horizon.bounds
    axes.set_xlim(west, east)
    axes.set_ylim(south, north)

    draw(axes, view, basemap(COASTLINE) if coastlines else None)
    axes.plot(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
        [centre[0]],
        [centre[1]],
        marker="+",
        color="black",
        markersize=9,
        zorder=CENTRE_LAYER,
    )

    annotate(figure, 0.94, title_of(circle), 12.0)
    # Wrapped rather than one line: at this width the citation runs off both edges of the figure,
    # which is an attribution nobody can read and so not an attribution.
    annotate(figure, 0.055, textwrap.fill(CITATION, width=118), 6.5)
    return figure


def parse(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Render a circle from a popcircles document.")
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--projection", choices=PROJECTIONS, default=PLATE_CARREE)
    parser.add_argument("--no-coastlines", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse(argv)
    input_path: Path = args.input
    output_path: Path = args.output
    projection: str = args.projection
    no_coastlines: bool = args.no_coastlines

    document = json.loads(input_path.read_text(encoding="utf-8"))
    figure = render(circle_of(document), projection, coastlines=not no_coastlines)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    # No bbox_inches="tight": the title and citation are placed at figure coordinates, and cropping
    # to the drawn content moves them off the positions they were given.
    figure.savefig(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
        output_path,
        dpi=200,
    )
    plt.close(figure)
    print(output_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
