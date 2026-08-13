// The decoder side of the seam: `tiff`, a path, and the tag validation. Everything above it in
// `raster.rs` stays free of both, which is what makes ADR 0002's fallback condition affordable.

// unwrap/expect are warn at workspace level and lint:rust runs --all-targets, so tests need this
// narrow exemption; docs/ai/code.md allows both in tests. float_cmp is here because a fixture's values
// must survive a round trip exactly: a tolerance would hide the byte-order and stride mistakes these
// tests exist to catch.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use std::io::Cursor;

    use tiff::decoder::{Decoder, DecodingResult};

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
