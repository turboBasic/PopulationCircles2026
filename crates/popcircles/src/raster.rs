// The raster seam: the vocabulary every consumer matches on, and no decoder. Which crate does the
// decoding is the sibling module's business and appears in no type here, so replacing it — ADR 0002
// names the conditions — is not a breaking change to anything downstream has matched on.

use std::error::Error;
use std::fmt;

use crate::geodesy::LatLon;
use crate::grid::Grid;

/// What the caller declares a raster must be, and what a reader hands back on success: the grid a
/// consumer sees is this one, never one assembled from the file's own tags. A geotransform arrives as
/// a decimal that need not round-trip to the rational it means — the registry raster's step is
/// `1/120 + 5.4e-16` — and every later stage wants the rational.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RasterSpec {
    pub grid: Grid,
    pub epsg: u16,
    pub pixel: PixelType,
    /// The sentinel the file must declare. Compared bit for bit rather than by value: the registry
    /// raster's sits two ulps off `-f32::MAX`, so a reader reaching for that constant matches nothing,
    /// counts no nodata, and reports a world population it has no reason to doubt.
    pub nodata: f32,
}

/// The one storage layout this reader accepts. An enum rather than a constant so that a second layout
/// arrives as a variant every `match` has to answer for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelType {
    Float32,
}

impl fmt::Display for PixelType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Float32 => f.write_str("Float32 (IEEE floating point, 32 bits)"),
        }
    }
}

/// Where every cell of a drained raster went. The four classes are disjoint and exhaustive, so they
/// sum to the grid's cell count, and that sum is the claim a caller can check its own traversal
/// against.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CellTallies {
    /// Cells matching the declared sentinel bit for bit.
    pub nodata: u64,
    /// Negative cells that did not match it. Zero population all the same, counted apart because
    /// nothing about the dataset says they should exist — the count is what turns that from an
    /// assumption into an observation.
    pub unexpected_negative: u64,
    pub zero: u64,
    pub populated: u64,
}

impl CellTallies {
    #[must_use]
    pub const fn total(self) -> u64 {
        self.nodata + self.unexpected_negative + self.zero + self.populated
    }
}

/// Every way a raster can fail to be the one that was declared.
///
/// A message names both sides — what was declared, or what this reader requires, and what the file
/// actually says. A rejection reading only "grid mismatch" sends its reader back to the file with a
/// hex editor, which is the failure this enum's shape is against.
#[derive(Debug, thiserror::Error)]
pub enum RasterError {
    #[error(
        "declared a {} x {} raster; the file is {} x {}",
        declared.0, declared.1, found.0, found.1
    )]
    Dimensions {
        declared: (u32, u32),
        found: (u32, u32),
    },

    #[error(
        "declared origin (lat {}, lon {}); the file's tiepoint is (lat {}, lon {}), outside the \
         tolerance a rounded geotransform decimal is allowed",
        declared.lat, declared.lon, found.lat, found.lon
    )]
    Origin { declared: LatLon, found: LatLon },

    #[error(
        "declared step (lon {}, lat {}); the file's pixel scale is (lon {}, lat {}), outside the \
         tolerance a rounded geotransform decimal is allowed",
        declared.0, declared.1, found.0, found.1
    )]
    Step {
        declared: (f64, f64),
        found: (f64, f64),
    },

    #[error(
        "this reader reads strips; the file is tiled (tags TileWidth 322 and TileLength 323 present)"
    )]
    Tiled,

    #[error("declared {declared} sample per pixel; the file says {found}")]
    SamplesPerPixel { declared: u16, found: u16 },

    #[error(
        "declared {declared}; the file says SampleFormat {found_sample_format} at {found_bits} bits \
         per sample"
    )]
    PixelFormat {
        declared: PixelType,
        found_sample_format: u16,
        found_bits: u16,
    },

    #[error(
        "this reader requires GTRasterType 1 (RasterPixelIsArea), which is what puts a tiepoint on \
         the outer corner of the first cell; the file says {found}"
    )]
    RasterType { found: u16 },

    #[error("this reader requires GTModelType 2 (geographic); the file says {found}")]
    ModelType { found: u16 },

    #[error("declared EPSG {declared}; the file's GeographicType key says {found}")]
    Epsg { declared: u16, found: u16 },

    #[error(
        "this reader reads a pixel scale and a tiepoint; the file carries ModelTransformation (tag \
         34264), which can express a rotation neither of those can"
    )]
    ModelTransformation,

    #[error(
        "declared nodata {declared:e} (0x{:08x}); the file declares {found:e} (0x{:08x})",
        declared.to_bits(), found.to_bits()
    )]
    NodataMismatch { declared: f32, found: f32 },

    #[error("declared nodata {declared:e}; the file's GDAL_NODATA tag reads {found:?}")]
    NodataNotANumber { declared: f32, found: String },

    #[error("declared nodata {declared:e}; the file carries no GDAL_NODATA tag (42113)")]
    NodataMissing { declared: f32 },

    #[error("the file carries no {name} tag ({tag}), so this reader cannot read its geotransform")]
    MissingTag { name: &'static str, tag: u16 },

    #[error("the file's GeoKeyDirectory carries no {name} key ({key})")]
    MissingGeoKey { name: &'static str, key: u16 },

    #[error(
        "the file is an unfetched Git LFS pointer rather than a raster; fetch it with `mise run \
         data:pull`"
    )]
    UnfetchedPointer,

    // Apart from Decode, and deliberately: a file that is not there and a file whose bytes are wrong
    // are different findings, and #8's exit-code classes will want to separate them. Folding both
    // into Decode would make "the decoder said no" mean "something went wrong".
    #[error("the raster could not be read")]
    Io(#[source] std::io::Error),

    // The source is boxed and opaque, so no consumer matches on the decoder's own error type and
    // ADR 0002's fallback stays the two lines of Cargo.toml it claims to be.
    #[error("the raster could not be decoded")]
    Decode(#[source] Box<dyn Error + Send + Sync>),
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this
// narrow exemption; docs/ai/code.md allows both in tests.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn spec_grid() -> Grid {
        Grid::new(
            43200,
            21600,
            LatLon {
                lat: 90.0,
                lon: -180.0,
            },
            1.0 / 120.0,
            -1.0 / 120.0,
        )
        .expect("the registry row is a valid grid")
    }

    // The sentinel the registry raster declares, which is not -f32::MAX: two ulps short of it, and
    // the bit pattern is what the assertions below pin.
    const GPW_NODATA: f32 = -3.402_823e38;

    #[test]
    fn the_spec_carries_the_declared_grid_verbatim() {
        let spec = RasterSpec {
            grid: spec_grid(),
            epsg: 4326,
            pixel: PixelType::Float32,
            nodata: GPW_NODATA,
        };
        assert_eq!(spec.grid, spec_grid());
        assert_eq!(spec.nodata.to_bits(), 0xff7f_fffd);
    }

    #[test]
    fn the_tallies_sum_to_the_cells_they_classify() {
        let tallies = CellTallies {
            nodata: 710_450_072,
            unexpected_negative: 0,
            zero: 40_311_312,
            populated: 182_358_616,
        };
        assert_eq!(tallies.total(), 933_120_000);
        assert_eq!(CellTallies::default().total(), 0);
    }

    type Rejection = (RasterError, Vec<&'static str>);

    // The three that compare a number the file carries against one the caller declared.
    fn geometry_rejections() -> Vec<Rejection> {
        vec![
            (
                RasterError::Dimensions {
                    declared: (43200, 21600),
                    found: (43200, 21601),
                },
                vec!["43200 x 21600", "43200 x 21601"],
            ),
            (
                RasterError::Origin {
                    declared: LatLon {
                        lat: 90.0,
                        lon: -180.0,
                    },
                    found: LatLon {
                        lat: 89.5,
                        lon: -180.0,
                    },
                },
                vec!["lat 90", "lat 89.5"],
            ),
            (
                RasterError::Step {
                    declared: (1.0 / 120.0, -1.0 / 120.0),
                    found: (-1.0 / 120.0, -1.0 / 120.0),
                },
                vec!["lon 0.008333333333333333", "lon -0.008333333333333333"],
            ),
        ]
    }

    // What the layout tags and the GeoKeys say.
    fn layout_rejections() -> Vec<Rejection> {
        vec![
            (RasterError::Tiled, vec!["strips", "tiled", "322"]),
            (
                RasterError::SamplesPerPixel {
                    declared: 1,
                    found: 3,
                },
                vec!["declared 1", "says 3"],
            ),
            (
                RasterError::PixelFormat {
                    declared: PixelType::Float32,
                    found_sample_format: 1,
                    found_bits: 32,
                },
                vec!["Float32", "SampleFormat 1", "32 bits"],
            ),
            (
                RasterError::RasterType { found: 2 },
                vec!["GTRasterType 1", "says 2"],
            ),
            (
                RasterError::ModelType { found: 1 },
                vec!["GTModelType 2", "says 1"],
            ),
            (
                RasterError::Epsg {
                    declared: 4326,
                    found: 3857,
                },
                vec!["EPSG 4326", "says 3857"],
            ),
            (
                RasterError::ModelTransformation,
                vec!["pixel scale", "34264"],
            ),
        ]
    }

    // The sentinel, the absences, and the three failures that are not a disagreement about the file's
    // contents at all.
    fn nodata_and_absence_rejections() -> Vec<Rejection> {
        vec![
            (
                RasterError::NodataMismatch {
                    declared: GPW_NODATA,
                    found: -9999.0,
                },
                vec!["0xff7ffffd", "-9.999e3", "0xc61c3c00"],
            ),
            (
                RasterError::NodataNotANumber {
                    declared: GPW_NODATA,
                    found: "not-a-number".to_owned(),
                },
                vec!["-3.402823e38", "not-a-number"],
            ),
            (
                RasterError::NodataMissing {
                    declared: GPW_NODATA,
                },
                vec!["-3.402823e38", "42113"],
            ),
            (
                RasterError::MissingTag {
                    name: "ModelPixelScale",
                    tag: 33550,
                },
                vec!["ModelPixelScale", "33550"],
            ),
            (
                RasterError::MissingGeoKey {
                    name: "GeographicType",
                    key: 2048,
                },
                vec!["GeographicType", "2048"],
            ),
            (
                RasterError::UnfetchedPointer,
                vec!["Git LFS pointer", "mise run data:pull"],
            ),
            (
                RasterError::Io(std::io::Error::from(std::io::ErrorKind::NotFound)),
                vec!["read"],
            ),
            (
                RasterError::Decode(Box::new(std::io::Error::from(
                    std::io::ErrorKind::UnexpectedEof,
                ))),
                vec!["decoded"],
            ),
        ]
    }

    // The substrings each message owes a reader: what was declared or required, and what the file
    // said. A table rather than a test each is what makes the two-sided property a property of the
    // enum instead of a habit of whoever wrote the last variant.
    #[test]
    fn every_rejection_names_both_sides() {
        let cases = geometry_rejections()
            .into_iter()
            .chain(layout_rejections())
            .chain(nodata_and_absence_rejections());

        for (error, wanted) in cases {
            let message = error.to_string();
            for fragment in wanted {
                assert!(
                    message.contains(fragment),
                    "{error:?} says {message:?}, which does not name {fragment:?}"
                );
            }
        }
    }

    #[test]
    fn the_error_crosses_a_thread_and_a_boxed_source_survives() {
        // Send + Sync is what lets a parallel search return one, and the source chain is what makes
        // the opaque Decode variant usable rather than merely opaque.
        fn assert_error<E: Error + Send + Sync + 'static>(error: E) -> String {
            error.source().map_or_else(String::new, ToString::to_string)
        }
        let inner = std::io::Error::from(std::io::ErrorKind::UnexpectedEof);
        let wanted = inner.to_string();
        assert_eq!(assert_error(RasterError::Decode(Box::new(inner))), wanted);
        assert!(assert_error(RasterError::Tiled).is_empty());
    }
}
