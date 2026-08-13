// The decoder side of the seam: `tiff`, a path, and the tag validation. Everything above it in
// `raster.rs` stays free of both, which is what makes ADR 0002's fallback condition affordable.

use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::Path;

use tiff::decoder::{ChunkType, Decoder, DecodingResult};
use tiff::tags::Tag;

use super::{
    CellTallies, PixelType, RasterError, RasterRow, RasterSource, RasterSpec, sanitise_row,
};
use crate::geodesy::{LatLon, wrap_lon};
use crate::grid::{BOUNDARY_TOLERANCE_DEG, Grid};

// GeoKeys, by number for the same reason the tags are.
const GT_MODEL_TYPE: u16 = 1024;
const GT_RASTER_TYPE: u16 = 1025;
const GEOGRAPHIC_TYPE: u16 = 2048;

const MODEL_TYPE_GEOGRAPHIC: u16 = 2;
const RASTER_PIXEL_IS_AREA: u16 = 1;
const SAMPLE_FORMAT_IEEEFP: u16 = 3;

/// A striped, single-band `GeoTIFF` validated against a [`RasterSpec`].
///
/// The declared grid wins: nothing here assembles a `Grid` from the file's tags. The file is required
/// to agree with the caller's, within the rounding a geotransform decimal is allowed, and the grid a
/// consumer sees is the caller's clean rational.
#[derive(Debug)]
pub struct GeoTiffSource<R: Read + Seek> {
    decoder: Decoder<R>,
    spec: RasterSpec,
    /// One strip of decoded values, which is the whole of this reader's resident memory: at the registry
    /// raster's one row per strip that is 173 KB against the file's 428 MB.
    strip: Vec<f32>,
    rows_buffered: u32,
    row_in_strip: u32,
    next_strip: u32,
    next_row: u32,
    tallies: CellTallies,
    /// A decode failure ends the stream. Without it a caller looping over `next_row` and logging
    /// errors rather than stopping would be handed the same failure for ever.
    failed: bool,
}

impl GeoTiffSource<BufReader<File>> {
    /// # Errors
    /// [`RasterError`] when the file cannot be read, cannot be decoded, or is not the raster `spec`
    /// declares.
    pub fn open(path: impl AsRef<Path>, spec: &RasterSpec) -> Result<Self, RasterError> {
        let file = File::open(path).map_err(RasterError::Io)?;
        Self::from_reader(BufReader::new(file), spec)
    }
}

impl<R: Read + Seek> GeoTiffSource<R> {
    fn from_reader(reader: R, spec: &RasterSpec) -> Result<Self, RasterError> {
        // The default Limits stand, per ADR 0002: the registry raster's 86 KB strip offsets and 173 KB
        // strips sit far inside them, so unlimited would only widen what a malformed header can ask for.
        let mut decoder = decoded(Decoder::new(reader))?;

        if decoder.get_chunk_type() == ChunkType::Tile {
            return Err(RasterError::Tiled);
        }

        // A missing SamplesPerPixel means one sample and a missing SampleFormat means unsigned integer,
        // both by TIFF 6.0, and a file relying on either default is a file this reader still has to
        // answer for.
        let samples = tag_u16(&mut decoder, Tag::SamplesPerPixel, 1)?;
        if samples != 1 {
            return Err(RasterError::SamplesPerPixel {
                declared: 1,
                found: samples,
            });
        }
        let sample_format = tag_u16(&mut decoder, Tag::SampleFormat, 1)?;
        let bits = tag_u16(&mut decoder, Tag::BitsPerSample, 1)?;
        if (sample_format, bits) != (SAMPLE_FORMAT_IEEEFP, 32) {
            return Err(RasterError::PixelFormat {
                declared: spec.pixel,
                found_sample_format: sample_format,
                found_bits: bits,
            });
        }
        // One layout, one variant, so the match is what will fail to compile when a second arrives.
        match spec.pixel {
            PixelType::Float32 => {}
        }

        check_geo_keys(&mut decoder, spec)?;
        check_nodata(&mut decoder, spec)?;
        check_geotransform(&mut decoder, spec)?;

        Ok(Self {
            decoder,
            spec: *spec,
            strip: Vec::new(),
            rows_buffered: 0,
            row_in_strip: 0,
            next_strip: 0,
            next_row: 0,
            tallies: CellTallies::default(),
            failed: false,
        })
    }

    /// Decode one strip, replacing whatever the last one left.
    ///
    /// `chunk_data_dimensions` is what makes a ragged last strip ordinary rather than a special case: it
    /// reports the rows the strip really holds, which is fewer than `RowsPerStrip` whenever the height is
    /// not a multiple of it.
    fn buffer_next_strip(&mut self) -> Result<(), RasterError> {
        let index = self.next_strip;
        let (_, rows) = self.decoder.chunk_data_dimensions(index);
        match decoded(self.decoder.read_chunk(index))? {
            DecodingResult::F32(values) => self.strip = values,
            other => {
                // The tags were checked to say IEEEFP at 32 bits, so this is the decoder disagreeing
                // with the header rather than a raster disagreeing with the caller — which is what the
                // opaque variant is for.
                return Err(RasterError::Decode(Box::new(UnexpectedBuffer {
                    found: buffer_kind(&other),
                })));
            }
        }
        self.rows_buffered = rows;
        self.row_in_strip = 0;
        self.next_strip += 1;
        Ok(())
    }
}

impl<R: Read + Seek> RasterSource for GeoTiffSource<R> {
    fn grid(&self) -> Grid {
        self.spec.grid
    }

    fn next_row(&mut self) -> Option<Result<RasterRow<'_>, RasterError>> {
        if self.failed {
            return None;
        }
        // The declared grid is the mint and the bound at once, so there is no second end-of-stream test
        // to keep in step with it.
        let row = self.spec.grid.row(self.next_row)?;

        if self.row_in_strip >= self.rows_buffered
            && let Err(error) = self.buffer_next_strip()
        {
            self.failed = true;
            return Some(Err(error));
        }

        // Both casts are of a value the grid or the strip already bounds, and usize is at least 32 bits
        // on every target this builds for.
        let width = self.spec.grid.width() as usize;
        let start = self.row_in_strip as usize * width;
        let end = start + width;
        if end > self.strip.len() {
            self.failed = true;
            return Some(Err(RasterError::Decode(Box::new(UnexpectedBuffer {
                found: "a strip shorter than its own row count",
            }))));
        }

        sanitise_row(
            &mut self.strip[start..end],
            self.spec.nodata,
            &mut self.tallies,
        );
        self.row_in_strip += 1;
        self.next_row += 1;
        Some(Ok(RasterRow {
            row,
            values: &self.strip[start..end],
        }))
    }

    fn finish(self) -> CellTallies {
        self.tallies
    }
}

/// The decoder contradicting the header it just parsed. Private and boxed into
/// [`RasterError::Decode`], so the shared enum gains no variant for a case no caller can act on
/// differently.
#[derive(Debug, thiserror::Error)]
#[error("the decoder produced {found} for a chunk whose tags declared Float32")]
struct UnexpectedBuffer {
    found: &'static str,
}

fn buffer_kind(result: &DecodingResult) -> &'static str {
    match result {
        DecodingResult::U8(_) => "8-bit unsigned values",
        DecodingResult::U16(_) => "16-bit unsigned values",
        DecodingResult::U32(_) => "32-bit unsigned values",
        DecodingResult::U64(_) => "64-bit unsigned values",
        DecodingResult::I8(_) => "8-bit signed values",
        DecodingResult::I16(_) => "16-bit signed values",
        DecodingResult::I32(_) => "32-bit signed values",
        DecodingResult::I64(_) => "64-bit signed values",
        DecodingResult::F16(_) => "16-bit floating point values",
        DecodingResult::F32(_) => "32-bit floating point values",
        DecodingResult::F64(_) => "64-bit floating point values",
    }
}

/// Every `tiff` failure becomes the one opaque variant, converted here at the boundary rather than
/// carried into the shared enum: a `#[from]` there would make ADR 0002's decoder swap a breaking
/// change to vocabulary the rest of the search has already matched on.
fn decoded<T>(result: tiff::TiffResult<T>) -> Result<T, RasterError> {
    result.map_err(|error| RasterError::Decode(Box::new(error)))
}

fn tag_u16<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    tag: Tag,
    absent: u16,
) -> Result<u16, RasterError> {
    // The first value only: SamplesPerPixel is scalar, and BitsPerSample and SampleFormat carry one
    // entry per sample, of which this reader accepts exactly one.
    let values = decoded(decoder.find_tag_unsigned_vec::<u16>(tag))?;
    Ok(values
        .and_then(|values| values.first().copied())
        .unwrap_or(absent))
}

/// `GTModelType`, `GTRasterType` and `GeographicType`, from the key directory's flat short array.
///
/// `RasterPixelIsArea` is not a formality: it is what puts the tiepoint on the outer corner of the
/// first cell, which is exactly where [`crate::grid::Grid`] assumes its origin sits. A
/// `RasterPixelIsPoint` file needs a half-cell shift this reader does not apply, so the key is checked
/// rather than assumed. The ellipsoidal keys the registry raster also carries are read past: the earth
/// model is `geodesy.rs`'s sphere and a raster does not get to change it.
fn check_geo_keys<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    spec: &RasterSpec,
) -> Result<(), RasterError> {
    let directory = decoded(decoder.find_tag_unsigned_vec::<u16>(Tag::GeoKeyDirectoryTag))?.ok_or(
        RasterError::MissingTag {
            name: "GeoKeyDirectory",
            tag: 34735,
        },
    )?;

    let model_type = geo_key(&directory, GT_MODEL_TYPE).ok_or(RasterError::MissingGeoKey {
        name: "GTModelType",
        key: GT_MODEL_TYPE,
    })?;
    if model_type != MODEL_TYPE_GEOGRAPHIC {
        return Err(RasterError::ModelType { found: model_type });
    }

    let raster_type = geo_key(&directory, GT_RASTER_TYPE).ok_or(RasterError::MissingGeoKey {
        name: "GTRasterType",
        key: GT_RASTER_TYPE,
    })?;
    if raster_type != RASTER_PIXEL_IS_AREA {
        return Err(RasterError::RasterType { found: raster_type });
    }

    let epsg = geo_key(&directory, GEOGRAPHIC_TYPE).ok_or(RasterError::MissingGeoKey {
        name: "GeographicType",
        key: GEOGRAPHIC_TYPE,
    })?;
    if epsg != spec.epsg {
        return Err(RasterError::Epsg {
            declared: spec.epsg,
            found: epsg,
        });
    }
    Ok(())
}

/// The key directory is a four-short header — version, revision, minor revision, key count — followed
/// by four shorts per key: the key, where its value lives, how many values, and the value itself when
/// the location is 0. Only location 0 is read: the three keys this reader wants are all shorts, and a
/// key deferred into another tag is a file it would reject on the value anyway.
fn geo_key(directory: &[u16], key: u16) -> Option<u16> {
    let count = usize::from(*directory.get(3)?);
    (0..count)
        .filter_map(|index| directory.get(4 + index * 4..8 + index * 4))
        .find(|entry| entry[0] == key && entry[1] == 0 && entry[2] == 1)
        .map(|entry| entry[3])
}

/// The sentinel, compared on bits against the declared one — the two-ulp trap `RasterSpec::nodata`
/// describes. The tag is ASCII, so a file can carry anything at all in it, including `nan`, which is
/// legal and which `==` would never match.
fn check_nodata<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    spec: &RasterSpec,
) -> Result<(), RasterError> {
    let text = decoded(decoder.find_tag(Tag::GdalNodata))?.ok_or(RasterError::NodataMissing {
        declared: spec.nodata,
    })?;
    let text = decoded(text.into_string())?;
    // The tag is NUL-terminated in the file and the decoder may hand the terminator back with it.
    let text = text.trim_end_matches('\0').trim();

    let found: f32 = text.parse().map_err(|_| RasterError::NodataNotANumber {
        declared: spec.nodata,
        found: text.to_owned(),
    })?;
    if found.to_bits() != spec.nodata.to_bits() {
        return Err(RasterError::NodataMismatch {
            declared: spec.nodata,
            found,
        });
    }
    Ok(())
}

/// The file's dimensions, origin and step against the declared grid's.
///
/// Both longitudes are reduced modulo a full turn before comparison. `Grid::new` canonicalises its
/// origin longitude, so a caller declaring 180 holds a grid reporting -180, and a tiepoint of 180 would
/// otherwise miss it by exactly 360 — a rejection of a file that agrees. Latitude takes no such
/// reduction: a latitude past a pole is an error rather than a value to fold, which is why `geodesy`
/// offers `wrap_lon` and nothing for the other axis.
fn check_geotransform<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    spec: &RasterSpec,
) -> Result<(), RasterError> {
    let found_dimensions = decoded(decoder.dimensions())?;
    let declared_dimensions = (spec.grid.width(), spec.grid.height());
    if found_dimensions != declared_dimensions {
        return Err(RasterError::Dimensions {
            declared: declared_dimensions,
            found: found_dimensions,
        });
    }

    let scale = geotransform_tag(
        decoder,
        Tag::ModelPixelScaleTag,
        "ModelPixelScale",
        33550,
        3,
    )?;
    let tiepoint = geotransform_tag(decoder, Tag::ModelTiepointTag, "ModelTiepoint", 33922, 6)?;
    if decoded(decoder.find_tag(Tag::ModelTransformationTag))?.is_some() {
        return Err(RasterError::ModelTransformation);
    }

    // ModelPixelScale carries magnitudes, and a north-up raster's rows run southward, so the latitude
    // step is the negation of the second — the sign convention `Grid` documents and keeps.
    let found_step = (scale[0], -scale[1]);
    let declared_step = (spec.grid.lon_step(), spec.grid.lat_step());
    if (found_step.0 - declared_step.0).abs() > BOUNDARY_TOLERANCE_DEG
        || (found_step.1 - declared_step.1).abs() > BOUNDARY_TOLERANCE_DEG
    {
        return Err(RasterError::Step {
            declared: declared_step,
            found: found_step,
        });
    }

    // A tiepoint ties a raster point to a world point, and need not be the first cell's corner. Walking
    // back from it costs two multiplications and is what makes the check about the grid rather than
    // about where the file chose to anchor it.
    let found_origin = LatLon {
        lat: tiepoint[4] + tiepoint[1] * scale[1],
        lon: tiepoint[3] - tiepoint[0] * scale[0],
    };
    let declared_origin = spec.grid.origin();
    if (found_origin.lat - declared_origin.lat).abs() > BOUNDARY_TOLERANCE_DEG
        || wrap_lon(found_origin.lon - declared_origin.lon).abs() > BOUNDARY_TOLERANCE_DEG
    {
        return Err(RasterError::Origin {
            declared: declared_origin,
            found: found_origin,
        });
    }
    Ok(())
}

fn geotransform_tag<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    tag: Tag,
    name: &'static str,
    number: u16,
    wanted: usize,
) -> Result<Vec<f64>, RasterError> {
    let values =
        decoded(decoder.find_tag(tag))?.ok_or(RasterError::MissingTag { name, tag: number })?;
    let values = decoded(values.into_f64_vec())?;
    if values.len() < wanted {
        // Too few values is the tag not being what it claims to be, which is the decoder's business
        // rather than a disagreement about the grid.
        return Err(RasterError::MissingTag { name, tag: number });
    }
    Ok(values)
}

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this
// narrow exemption; docs/ai/code.md allows both in tests. float_cmp is here because a fixture's values
// must survive a round trip exactly: a tolerance would hide the byte-order and stride mistakes these
// tests exist to catch.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use std::io::Cursor;

    use tiff::decoder::DecodingResult;

    use super::*;
    use crate::grid::{Grid, Row};

    // TIFF 6.0 tags, by number, because that is how they appear in a hex dump of a file that disagrees
    // with us.
    const IMAGE_WIDTH: u16 = 256;
    const IMAGE_LENGTH: u16 = 257;
    const BITS_PER_SAMPLE: u16 = 258;
    const COMPRESSION: u16 = 259;
    const PHOTOMETRIC: u16 = 262;
    const STRIP_OFFSETS: u16 = 273;
    const SAMPLES_PER_PIXEL: u16 = 277;
    const ROWS_PER_STRIP: u16 = 278;
    const STRIP_BYTE_COUNTS: u16 = 279;
    const PLANAR_CONFIG: u16 = 284;
    const TILE_WIDTH: u16 = 322;
    const TILE_LENGTH: u16 = 323;
    const TILE_OFFSETS: u16 = 324;
    const TILE_BYTE_COUNTS: u16 = 325;
    const SAMPLE_FORMAT: u16 = 339;
    const MODEL_PIXEL_SCALE: u16 = 33550;
    const MODEL_TIEPOINT: u16 = 33922;
    const MODEL_TRANSFORMATION: u16 = 34264;
    const GEO_KEY_DIRECTORY: u16 = 34735;
    const GDAL_NODATA: u16 = 42113;

    const TYPE_ASCII: u16 = 2;
    const TYPE_SHORT: u16 = 3;
    const TYPE_LONG: u16 = 4;
    const TYPE_DOUBLE: u16 = 12;

    const COMPRESSION_NONE: u16 = 1;
    const COMPRESSION_LZW: u16 = 5;

    /// One IFD entry, with its value already in bytes. Four bytes or fewer live in the entry itself;
    /// anything longer is placed after the directory and referenced by offset, which is the rule
    /// [`Fixture::bytes`] implements.
    #[derive(Debug, Clone)]
    struct Entry {
        tag: u16,
        field_type: u16,
        count: u32,
        payload: Vec<u8>,
    }

    impl Entry {
        fn short(tag: u16, values: &[u16]) -> Self {
            let mut payload = Vec::with_capacity(values.len() * 2);
            for value in values {
                payload.extend_from_slice(&value.to_le_bytes());
            }
            Self::new(tag, TYPE_SHORT, values.len(), payload)
        }

        fn long(tag: u16, values: &[u32]) -> Self {
            let mut payload = Vec::with_capacity(values.len() * 4);
            for value in values {
                payload.extend_from_slice(&value.to_le_bytes());
            }
            Self::new(tag, TYPE_LONG, values.len(), payload)
        }

        fn double(tag: u16, values: &[f64]) -> Self {
            let mut payload = Vec::with_capacity(values.len() * 8);
            for value in values {
                payload.extend_from_slice(&value.to_le_bytes());
            }
            Self::new(tag, TYPE_DOUBLE, values.len(), payload)
        }

        // ASCII counts the terminating NUL, which is the mistake a reader of the specification makes
        // once.
        fn ascii(tag: u16, text: &str) -> Self {
            let mut payload = text.as_bytes().to_vec();
            payload.push(0);
            let count = payload.len();
            Self::new(tag, TYPE_ASCII, count, payload)
        }

        fn new(tag: u16, field_type: u16, count: usize, payload: Vec<u8>) -> Self {
            Self {
                tag,
                field_type,
                count: u32::try_from(count).unwrap(),
                payload,
            }
        }
    }

    /// The three `GeoKeys` this reader looks at. `None` omits the key, which is a file a real reader has
    /// to answer for and not a case a well-behaved writer would produce.
    #[derive(Debug, Clone, Copy)]
    struct GeoKeys {
        model_type: Option<u16>,
        raster_type: Option<u16>,
        epsg: Option<u16>,
    }

    impl Default for GeoKeys {
        fn default() -> Self {
            Self {
                model_type: Some(2),
                raster_type: Some(1),
                epsg: Some(4326),
            }
        }
    }

    impl GeoKeys {
        // The GeoTIFF key directory: a four-short header (1, 1, 0, key count) then four shorts per key.
        // A TIFFTagLocation of 0 means the value sits in the fourth short rather than in another tag,
        // which is how every key here is stored.
        fn directory(self) -> Vec<u16> {
            let keys: Vec<(u16, u16)> = [
                (1024, self.model_type),
                (1025, self.raster_type),
                (2048, self.epsg),
            ]
            .into_iter()
            .filter_map(|(key, value)| value.map(|value| (key, value)))
            .collect();

            let mut out = vec![1, 1, 0, u16::try_from(keys.len()).unwrap()];
            for (key, value) in keys {
                out.extend_from_slice(&[key, 0, 1, value]);
            }
            out
        }
    }

    /// A TIFF written from the specification rather than by an encoder, because `tiff`'s encoder has no
    /// float colour type at all — the crate that decodes our raster cannot write one — and because a
    /// rejection test needs files no well-behaved encoder would emit.
    ///
    /// Every field is overridable through struct update syntax, and each override is a named rejection
    /// somewhere in this module's tests.
    #[derive(Debug, Clone)]
    struct Fixture {
        width: u32,
        height: u32,
        rows_per_strip: u32,
        values: Vec<f32>,
        bits_per_sample: u16,
        sample_format: u16,
        samples_per_pixel: u16,
        photometric: u16,
        compression: u16,
        pixel_scale: Option<[f64; 3]>,
        tiepoint: Option<[f64; 6]>,
        model_transformation: Option<[f64; 16]>,
        geo_keys: Option<GeoKeys>,
        nodata: Option<String>,
        tiled: bool,
        /// Cut bytes off the end while the byte counts still claim them: a decoder failure, which is a
        /// different finding from a rejection.
        truncated: bool,
    }

    impl Fixture {
        /// The shape of the registry raster in miniature: Float32, striped one row at a time, LZW, and
        /// the clean 1/120 grid from (-180, 90) rather than the file's rounded decimals.
        fn new(width: u32, height: u32, values: Vec<f32>) -> Self {
            assert_eq!(
                values.len(),
                (width * height) as usize,
                "a fixture's values are its whole raster"
            );
            let step = 1.0 / 120.0;
            Self {
                width,
                height,
                rows_per_strip: 1,
                values,
                bits_per_sample: 32,
                sample_format: 3,
                samples_per_pixel: 1,
                photometric: 1,
                compression: COMPRESSION_LZW,
                pixel_scale: Some([step, step, 0.0]),
                tiepoint: Some([0.0, 0.0, 0.0, -180.0, 90.0, 0.0]),
                model_transformation: None,
                geo_keys: Some(GeoKeys::default()),
                nodata: Some("-3.40282306073709653e+38".to_owned()),
                tiled: false,
                truncated: false,
            }
        }

        fn rows(&self) -> Vec<&[f32]> {
            self.values.chunks(self.width as usize).collect()
        }

        // Strip data, compressed the way the Compression tag says. weezl is what `tiff` itself decodes
        // LZW with, so a fixture is compressed by the implementation that will decompress it.
        fn strips(&self) -> Vec<Vec<u8>> {
            let mut strips = Vec::new();
            for chunk in self.rows().chunks(self.rows_per_strip as usize) {
                let mut raw = Vec::new();
                for row in chunk {
                    for value in *row {
                        raw.extend_from_slice(&value.to_le_bytes());
                    }
                }
                strips.push(if self.compression == COMPRESSION_LZW {
                    weezl::encode::Encoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8)
                        .encode(&raw)
                        .expect("in-memory LZW encoding of a fixture cannot fail")
                } else {
                    raw
                });
            }
            strips
        }

        // A tiled fixture carries tiles this reader never decodes — it rejects the layout first — so the
        // data is zeros of the right size, and the tile edge is 16 as the specification requires.
        fn tiles(&self) -> (u32, Vec<Vec<u8>>) {
            let edge = 16u32;
            let across = self.width.div_ceil(edge);
            let down = self.height.div_ceil(edge);
            let bytes = (edge * edge * u32::from(self.bits_per_sample) / 8) as usize;
            (edge, vec![vec![0u8; bytes]; (across * down) as usize])
        }

        fn bytes(&self) -> Vec<u8> {
            let samples = usize::from(self.samples_per_pixel);
            let mut entries = vec![
                Entry::long(IMAGE_WIDTH, &[self.width]),
                Entry::long(IMAGE_LENGTH, &[self.height]),
                Entry::short(BITS_PER_SAMPLE, &vec![self.bits_per_sample; samples]),
                Entry::short(COMPRESSION, &[self.compression]),
                Entry::short(PHOTOMETRIC, &[self.photometric]),
                Entry::short(SAMPLES_PER_PIXEL, &[self.samples_per_pixel]),
                Entry::short(PLANAR_CONFIG, &[1]),
                Entry::short(SAMPLE_FORMAT, &vec![self.sample_format; samples]),
            ];

            let (chunks, offsets_tag) = if self.tiled {
                let (edge, tiles) = self.tiles();
                let counts: Vec<u32> = tiles
                    .iter()
                    .map(|tile| u32::try_from(tile.len()).unwrap())
                    .collect();
                entries.push(Entry::short(TILE_WIDTH, &[u16::try_from(edge).unwrap()]));
                entries.push(Entry::short(TILE_LENGTH, &[u16::try_from(edge).unwrap()]));
                entries.push(Entry::long(TILE_OFFSETS, &vec![0; tiles.len()]));
                entries.push(Entry::long(TILE_BYTE_COUNTS, &counts));
                (tiles, TILE_OFFSETS)
            } else {
                let strips = self.strips();
                let counts: Vec<u32> = strips
                    .iter()
                    .map(|strip| u32::try_from(strip.len()).unwrap())
                    .collect();
                entries.push(Entry::long(ROWS_PER_STRIP, &[self.rows_per_strip]));
                entries.push(Entry::long(STRIP_OFFSETS, &vec![0; strips.len()]));
                entries.push(Entry::long(STRIP_BYTE_COUNTS, &counts));
                (strips, STRIP_OFFSETS)
            };

            if let Some(scale) = self.pixel_scale {
                entries.push(Entry::double(MODEL_PIXEL_SCALE, &scale));
            }
            if let Some(tiepoint) = self.tiepoint {
                entries.push(Entry::double(MODEL_TIEPOINT, &tiepoint));
            }
            if let Some(transformation) = self.model_transformation {
                entries.push(Entry::double(MODEL_TRANSFORMATION, &transformation));
            }
            if let Some(keys) = self.geo_keys {
                entries.push(Entry::short(GEO_KEY_DIRECTORY, &keys.directory()));
            }
            if let Some(nodata) = &self.nodata {
                entries.push(Entry::ascii(GDAL_NODATA, nodata));
            }

            assemble(entries, &chunks, offsets_tag, self.truncated)
        }
    }

    /// Header, directory, out-of-line values, then chunk data — and the offsets patched in once the
    /// layout is known, because a chunk offset is a fact about the file rather than about the raster.
    fn assemble(
        mut entries: Vec<Entry>,
        chunks: &[Vec<u8>],
        offsets_tag: u16,
        truncated: bool,
    ) -> Vec<u8> {
        // The specification requires ascending tag order, and a decoder is entitled to binary search.
        entries.sort_by_key(|entry| entry.tag);

        let directory_size = 2 + 12 * entries.len() + 4;
        let mut cursor = 8 + directory_size;
        let placement: Vec<Option<usize>> = entries
            .iter()
            .map(|entry| {
                if entry.payload.len() <= 4 {
                    return None;
                }
                let at = cursor;
                // Every out-of-line value starts on an even boundary, as the specification requires.
                cursor += entry.payload.len() + entry.payload.len() % 2;
                Some(at)
            })
            .collect();

        let mut at = cursor;
        let mut offsets = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            offsets.push(u32::try_from(at).unwrap());
            at += chunk.len();
        }
        let index = entries
            .iter()
            .position(|entry| entry.tag == offsets_tag)
            .expect("a fixture carries either strip offsets or tile offsets");
        entries[index] = Entry::long(offsets_tag, &offsets);

        let mut out = Vec::with_capacity(at);
        out.extend_from_slice(b"II");
        out.extend_from_slice(&42u16.to_le_bytes());
        out.extend_from_slice(&8u32.to_le_bytes());
        out.extend_from_slice(&u16::try_from(entries.len()).unwrap().to_le_bytes());
        for (entry, place) in entries.iter().zip(&placement) {
            out.extend_from_slice(&entry.tag.to_le_bytes());
            out.extend_from_slice(&entry.field_type.to_le_bytes());
            out.extend_from_slice(&entry.count.to_le_bytes());
            if let Some(offset) = place {
                out.extend_from_slice(&u32::try_from(*offset).unwrap().to_le_bytes());
            } else {
                // A value short enough to fit is left-justified in the four bytes on a little-endian
                // file, which is why the padding goes after it.
                let mut inline = entry.payload.clone();
                inline.resize(4, 0);
                out.extend_from_slice(&inline);
            }
        }
        out.extend_from_slice(&0u32.to_le_bytes());

        for (entry, place) in entries.iter().zip(&placement) {
            if place.is_some() {
                out.extend_from_slice(&entry.payload);
                if out.len() % 2 == 1 {
                    out.push(0);
                }
            }
        }
        // The placement arithmetic and the writing have to agree, or every offset above is a plausible
        // wrong number and the decode fails somewhere far from here.
        assert_eq!(
            out.len(),
            cursor,
            "the directory's offsets missed the values"
        );

        for chunk in chunks {
            out.extend_from_slice(chunk);
        }
        if truncated {
            let keep = out.len() - chunks.last().map_or(0, |chunk| chunk.len() / 2 + 1);
            out.truncate(keep);
        }
        out
    }

    // A ramp with a sentinel, a zero and a negative in it: distinct values in a known order, so a
    // transposition or a stride mistake shows up as a wrong number rather than as a wrong total.
    fn sample_values(width: u32, height: u32) -> Vec<f32> {
        (0..width * height)
            .map(|index| match index {
                0 => -3.402_823e38,
                1 => 0.0,
                2 => -1.0,
                other => f32::from(u16::try_from(other).unwrap()) + 0.25,
            })
            .collect()
    }

    fn decode_all(bytes: &[u8]) -> Vec<f32> {
        let mut decoder = Decoder::new(Cursor::new(bytes)).expect("the fixture is a readable TIFF");
        let mut out = Vec::new();
        for chunk in 0..decoder.strip_count().expect("a striped fixture has strips") {
            match decoder.read_chunk(chunk).expect("the strip decodes") {
                DecodingResult::F32(values) => out.extend_from_slice(&values),
                other => panic!("a Float32 fixture decoded as {other:?}"),
            }
        }
        out
    }

    // The clean rational grid a caller declares, which is never the decimal the file carries.
    fn declared_grid(width: u32, height: u32, origin_lon: f64) -> Grid {
        Grid::new(
            width,
            height,
            LatLon {
                lat: 90.0,
                lon: origin_lon,
            },
            1.0 / 120.0,
            -1.0 / 120.0,
        )
        .expect("a window at the north pole is a valid grid")
    }

    fn spec_for(grid: Grid) -> RasterSpec {
        RasterSpec {
            grid,
            epsg: 4326,
            pixel: PixelType::Float32,
            nodata: -3.402_823e38,
        }
    }

    fn open(
        fixture: &Fixture,
        spec: &RasterSpec,
    ) -> Result<GeoTiffSource<Cursor<Vec<u8>>>, RasterError> {
        GeoTiffSource::from_reader(Cursor::new(fixture.bytes()), spec)
    }

    #[test]
    fn the_real_rasters_own_geotransform_is_accepted() {
        // The numbers the registry raster actually carries, against the clean 1/120 grid from
        // (-180, 90): the step is 1/120 + 5.4e-16 and the origin latitude 90 + 1.16e-11. An exact
        // comparison passes every fixture an author would think to write and rejects this one, which is
        // why the assertion is written before the comparison it constrains.
        let fixture = Fixture {
            pixel_scale: Some([0.008_333_333_333_333_87, 0.008_333_333_333_333_87, 0.0]),
            tiepoint: Some([0.0, 0.0, 0.0, -180.0, 90.000_000_000_011_57, 0.0]),
            ..Fixture::new(4, 3, sample_values(4, 3))
        };
        let spec = spec_for(declared_grid(4, 3, -180.0));
        let mut source = open(&fixture, &spec).expect("the real raster's own numbers are accepted");

        // The grid it kept is the caller's rational, not the file's decimal.
        assert_eq!(source.spec.grid, spec.grid);
        assert_eq!(
            source.decoder.strip_count().expect("a striped fixture"),
            fixture.height
        );
    }

    #[test]
    fn a_tiepoint_on_the_far_side_of_the_seam_agrees_with_the_grid() {
        // `Grid::new` canonicalises its origin longitude, so a grid declared at 180 reports -180. A
        // tiepoint of 180 against it is a file that agrees, and an unreduced comparison misses it by
        // exactly a full turn. This raster says -180 on both sides, so nothing measured from it would
        // have caught the case.
        let fixture = Fixture {
            tiepoint: Some([0.0, 0.0, 0.0, 180.0, 90.0, 0.0]),
            ..Fixture::new(4, 3, sample_values(4, 3))
        };
        open(&fixture, &spec_for(declared_grid(4, 3, -180.0)))
            .expect("180 and -180 are the same meridian");
    }

    #[test]
    fn a_tiepoint_anchored_away_from_the_first_cell_is_walked_back() {
        // A tiepoint ties any raster point to a world point. Two cells east and one row south of the
        // corner is the same grid, stated differently.
        let step = 1.0 / 120.0;
        let fixture = Fixture {
            tiepoint: Some([2.0, 1.0, 0.0, -180.0 + 2.0 * step, 90.0 - step, 0.0]),
            ..Fixture::new(4, 3, sample_values(4, 3))
        };
        open(&fixture, &spec_for(declared_grid(4, 3, -180.0)))
            .expect("an anchor inside the raster describes the same grid");
    }

    // Every row the source hands out, and the tallies it ends with.
    fn drain(source: &mut GeoTiffSource<Cursor<Vec<u8>>>) -> Vec<(u32, Vec<f32>)> {
        let mut out = Vec::new();
        while let Some(row) = source.next_row() {
            let row = row.expect("a well-formed fixture streams without error");
            out.push((row.row.get(), row.values.to_vec()));
        }
        out
    }

    #[test]
    fn a_ragged_last_strip_reads_back_exactly() {
        // 5 x 7 at RowsPerStrip 3 leaves the last strip holding one row. That is where a reader trusting
        // the nominal strip height reads a row of somebody else's buffer, and the registry raster at one
        // row per strip never exercises it.
        let values = sample_values(5, 7);
        let fixture = Fixture {
            rows_per_strip: 3,
            ..Fixture::new(5, 7, values.clone())
        };
        let spec = spec_for(declared_grid(5, 7, -180.0));
        let mut source = open(&fixture, &spec).expect("the fixture matches the spec");

        let rows = drain(&mut source);
        assert_eq!(
            rows.iter().map(|(index, _)| *index).collect::<Vec<u32>>(),
            spec.grid.rows().map(Row::get).collect::<Vec<u32>>()
        );
        // Sanitised: the sentinel and the unexplained negative are zero, everything else verbatim.
        let expected: Vec<f32> = values
            .iter()
            .map(|value| if *value > 0.0 { *value } else { 0.0 })
            .collect();
        let read: Vec<f32> = rows.iter().flat_map(|(_, row)| row.clone()).collect();
        assert_eq!(read, expected);

        // Hand count of sample_values: cell 0 is the sentinel, cell 1 a zero, cell 2 a negative, and the
        // remaining 32 are counts.
        assert_eq!(
            source.finish(),
            CellTallies {
                nodata: 1,
                unexpected_negative: 1,
                zero: 1,
                populated: 32,
            }
        );
    }

    #[test]
    fn the_strip_height_changes_nothing_a_consumer_sees() {
        let values = sample_values(5, 7);
        let base = Fixture::new(5, 7, values);
        let spec = spec_for(declared_grid(5, 7, -180.0));

        let mut reference = None;
        for rows_per_strip in [1, 3, 7] {
            let fixture = Fixture {
                rows_per_strip,
                ..base.clone()
            };
            let mut source = open(&fixture, &spec).expect("the fixture matches the spec");
            let rows = drain(&mut source);
            let tallies = source.finish();
            match &reference {
                None => reference = Some((rows, tallies)),
                Some(first) => assert_eq!(
                    &(rows, tallies),
                    first,
                    "RowsPerStrip {rows_per_strip} read differently"
                ),
            }
        }
    }

    #[test]
    fn resident_memory_is_one_strip_rather_than_one_raster() {
        // Asserted on the buffer rather than claimed in a comment: at 3 rows of 5 the strip holds 15
        // values, not the raster's 35.
        let rows_per_strip = 3u32;
        let fixture = Fixture {
            rows_per_strip,
            ..Fixture::new(5, 7, sample_values(5, 7))
        };
        let spec = spec_for(declared_grid(5, 7, -180.0));
        let mut source = open(&fixture, &spec).expect("the fixture matches the spec");

        let mut peak = 0;
        while let Some(row) = source.next_row() {
            row.expect("a well-formed fixture streams without error");
            peak = peak.max(source.strip.capacity());
        }
        let one_strip = (rows_per_strip * spec.grid.width()) as usize;
        assert!(
            peak <= one_strip,
            "buffered {peak} values where one strip is {one_strip}"
        );
        assert!(peak < (spec.grid.width() * spec.grid.height()) as usize);
    }

    #[test]
    fn a_truncated_strip_is_a_decode_failure_and_ends_the_stream() {
        // The one case that is the decoder's rather than the file's disagreement with the caller: bytes
        // the strip byte counts promise and the file does not carry.
        let fixture = Fixture {
            truncated: true,
            ..Fixture::new(5, 7, sample_values(5, 7))
        };
        let spec = spec_for(declared_grid(5, 7, -180.0));
        let mut source = open(&fixture, &spec).expect("a truncated file still has a valid header");

        let mut error = None;
        while let Some(row) = source.next_row() {
            if let Err(found) = row {
                error = Some(found);
                break;
            }
        }
        assert!(matches!(error, Some(RasterError::Decode(_))));
        // Terminal: a caller that logs the error and keeps polling is not handed it for ever.
        assert!(source.next_row().is_none());
    }

    #[test]
    fn a_fixture_round_trips_through_the_decoder_compressed_or_not() {
        let values = sample_values(4, 3);
        let compressed = Fixture::new(4, 3, values.clone());
        let plain = Fixture {
            compression: COMPRESSION_NONE,
            ..compressed.clone()
        };

        // The writer is proved by the decoder this crate is about to depend on, before any code of ours
        // trusts either: identical values, bit for bit, from two different compressions.
        let from_lzw = decode_all(&compressed.bytes());
        let from_plain = decode_all(&plain.bytes());
        assert_eq!(from_lzw.len(), values.len());
        for (index, value) in values.iter().enumerate() {
            assert_eq!(value.to_bits(), from_lzw[index].to_bits(), "cell {index}");
            assert_eq!(value.to_bits(), from_plain[index].to_bits(), "cell {index}");
        }
    }

    #[test]
    fn every_strip_layout_of_the_same_data_decodes_the_same() {
        // A ragged last strip is the case the registry raster never exercises: at RowsPerStrip 1 every
        // strip is full, and 2 over 3 rows is where a reader that trusts the nominal strip height reads
        // a row of somebody else's memory.
        let values = sample_values(4, 3);
        let base = Fixture::new(4, 3, values.clone());
        for rows_per_strip in [1, 2, 3, 7] {
            let fixture = Fixture {
                rows_per_strip,
                ..base.clone()
            };
            assert_eq!(
                decode_all(&fixture.bytes())
                    .iter()
                    .map(|value| value.to_bits())
                    .collect::<Vec<u32>>(),
                values
                    .iter()
                    .map(|value| value.to_bits())
                    .collect::<Vec<u32>>(),
                "RowsPerStrip {rows_per_strip}"
            );
        }
    }
}
