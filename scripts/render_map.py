import argparse
import json
import textwrap
from pathlib import Path
from typing import cast

import cartopy.crs as ccrs
import matplotlib as mpl
from cartopy.mpl.geoaxes import GeoAxes
from circle_document import Circle, circle_of
from circle_geometry import cap, drawn
from matplotlib import pyplot as plt
from matplotlib.figure import Figure

# Every figure this writes goes to a file, so an interactive backend is never wanted — and asking
# for one is what fails on a machine with no display, rather than falling back.
mpl.use("Agg")

PLATE_CARREE = "plate-carree"
ORTHOGRAPHIC = "orthographic"
PROJECTIONS = (PLATE_CARREE, ORTHOGRAPHIC)

# CC BY 4.0 requires attribution of anything published from the raster, and `data/README.md`
# "Licence and attribution" is the owner of this text — a figure carrying a different wording is
# drift, which is what tests/test_render_map.py fails on.
CITATION = (
    "Center for International Earth Science Information Network — CIESIN — Columbia University. "
    "2018. Gridded Population of the World, Version 4 (GPWv4): Population Count Adjusted to Match "
    "2015 Revision of UN WPP Country Totals, Revision 11. Palisades, NY: NASA Socioeconomic Data "
    "and Applications Center (SEDAC). https://doi.org/10.7927/H4PN93PB"
)


def axes_projection(name: str, circle: Circle, globe: ccrs.Globe) -> ccrs.Projection:
    # Both are built on the cap's own globe: a projection on another sphere makes PROJ shift the
    # datum under the polygon, which is a second earth model arriving without anyone choosing it.
    if name == ORTHOGRAPHIC:
        return ccrs.Orthographic(
            central_longitude=circle.centre.lon,
            central_latitude=circle.centre.lat,
            globe=globe,
        )
    return ccrs.PlateCarree(globe=globe)


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


def render(circle: Circle, projection: str, *, coastlines: bool) -> Figure:
    built = cap(circle.centre, circle.radius_km, circle.earth_radius_km)
    target = ccrs.PlateCarree(globe=built.globe)

    figure = plt.figure(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
        figsize=(11.0, 6.0),
    )
    # Room for the title above and the wrapped citation below, since both are placed at figure
    # coordinates rather than left to a layout engine.
    figure.subplots_adjust(left=0.03, right=0.97, top=0.87, bottom=0.16)
    # matplotlib annotates add_subplot(projection=...) as returning Axes3D, so the cast is what
    # names what cartopy actually hands back. It is a cast to a checked type, not to Any.
    axes = cast(
        GeoAxes,
        figure.add_subplot(1, 1, 1, projection=axes_projection(projection, circle, built.globe)),
    )
    axes.set_global()
    if coastlines:
        # Downloads Natural Earth on first use, which is why the caller decides and why the one test
        # that asks for it is marked `network`.
        axes.coastlines(resolution="110m", color="dimgrey")
    axes.gridlines(draw_labels=False, linewidth=0.3)

    # The drawn polygon, not the ring: PROJ has already cut it at the seam and closed it over a
    # pole, which is the whole reason a ring of coordinates is not the drawing path.
    axes.add_geometries(
        [drawn(built, target)],
        target,
        facecolor="crimson",
        edgecolor="darkred",
        alpha=0.45,
        linewidth=0.8,
        zorder=3,
    )
    axes.plot(  # pyright: ignore[reportUnknownMemberType] — matplotlib **kwargs: Unknown
        [circle.centre.lon],
        [circle.centre.lat],
        marker="+",
        color="black",
        markersize=9,
        transform=target,
        zorder=4,
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

    # The one file this program opens. Nothing here reaches the raster, the table or the ledger.
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
