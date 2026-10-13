//! Hermetic behaviour of `supercom-scmap` over maps built in code. No game
//! is needed and no fixture file is committed: every map here is assembled
//! from literals, section by section, in the order of the format described
//! in the crate documentation.
//!
//! These pin what the installed game cannot: the exact error for each way a
//! map can be wrong, the walk through every section between the heightmap
//! and the strata, and the meaning of every field of a minimal map. The
//! crate is a skeleton, so each test below fails today on
//! `Error::Unimplemented` and passes only when the parser exists.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use supercom_scmap::{Error, Heightmap, Map, Stratum};

/// Bytes in the fixed header: signature, major version, two constant words,
/// width, height, a zero `i32`, a zero `i16`.
const HEADER_LEN: usize = 30;
/// Terrain strata in every map.
const STRATA: usize = 10;
/// Normal-map slots that follow the strata.
const NORMALS: usize = 9;

fn put_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_f32s(out: &mut Vec<u8>, values: &[f32]) {
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
}

fn put_cstr(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(text.as_bytes());
    out.push(0);
}

fn len32(len: usize) -> i32 {
    i32::try_from(len).expect("test buffers are far below 2 GiB")
}

/// Overwrites the little-endian `i32` at `at`.
fn set_i32(buf: &mut [u8], at: usize, value: i32) {
    buf[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// The fields of a synthetic map, before packing.
struct Spec {
    version: i32,
    width: f32,
    height: f32,
    preview: Vec<u8>,
    hm_width: i32,
    hm_height: i32,
    scale: f32,
    samples: Vec<i16>,
    cube_maps: usize,
    wave_generators: usize,
    strata: Vec<(&'static str, f32)>,
    /// Surface, deep, and abyss elevations; `None` builds an absent block.
    water: Option<(f32, f32, f32)>,
}

/// A built map and the offsets the tests patch or cut at.
struct Built {
    bytes: Vec<u8>,
    version_at: usize,
    strata_end: usize,
}

/// Ten strata with `used` at the given slots and unused (empty) elsewhere.
fn strata(used: &[(usize, &'static str, f32)]) -> Vec<(&'static str, f32)> {
    let mut all = vec![("", 0.0); STRATA];
    for &(slot, path, scale) in used {
        all[slot] = (path, scale);
    }
    all
}

impl Spec {
    /// A version-60 map with a 1x1 heightmap (four samples), an 8-by-4 world,
    /// no cube maps or wave generators, and one stratum.
    fn minimal() -> Self {
        Self {
            version: 60,
            width: 8.0,
            height: 4.0,
            preview: b"DDS \x01\x02\x03\x04".to_vec(),
            hm_width: 1,
            hm_height: 1,
            scale: 0.5,
            samples: vec![-1, 2, 300, -400],
            cube_maps: 0,
            wave_generators: 0,
            strata: strata(&[(0, "/textures/test/a.dds", 4.0)]),
            water: Some((0.25, 0.25, 0.25)),
        }
    }

    /// The expected parse of the map this spec builds.
    fn expected(&self) -> Map {
        Map {
            width: self.width,
            height: self.height,
            version: u32::try_from(self.version).expect("a positive version"),
            preview: self.preview.clone(),
            heightmap: Heightmap {
                width: u32::try_from(self.hm_width).expect("a positive width"),
                height: u32::try_from(self.hm_height).expect("a positive height"),
                scale: self.scale,
                samples: self.samples.clone(),
            },
            strata: self
                .strata
                .iter()
                .map(|&(texture, scale)| Stratum {
                    texture: texture.to_owned(),
                    scale,
                })
                .collect(),
            // The parser does not yet return the water; the decode flips this.
            water: None,
        }
    }

    /// The file. Sections between the heightmap and the strata carry
    /// non-zero filler so that nothing but their structure locates the
    /// strata; the sections after the strata are present but unread.
    fn build(&self) -> Built {
        assert_eq!(self.strata.len(), STRATA);
        let mut out = Vec::new();
        out.extend_from_slice(&0x1a70_614d_u32.to_le_bytes());
        put_i32(&mut out, 2);
        put_i32(&mut out, -1_091_567_891);
        put_i32(&mut out, 2);
        put_f32s(&mut out, &[self.width, self.height]);
        put_i32(&mut out, 0);
        out.extend_from_slice(&0_i16.to_le_bytes());
        assert_eq!(out.len(), HEADER_LEN);

        put_i32(&mut out, len32(self.preview.len()));
        out.extend_from_slice(&self.preview);
        let version_at = out.len();
        put_i32(&mut out, self.version);

        put_i32(&mut out, self.hm_width);
        put_i32(&mut out, self.hm_height);
        put_f32s(&mut out, &[self.scale]);
        for sample in &self.samples {
            out.extend_from_slice(&sample.to_le_bytes());
        }
        out.push(0);

        for text in ["shader", "background", "skycube"] {
            put_cstr(&mut out, text);
        }
        put_i32(&mut out, len32(self.cube_maps));
        for _ in 0..self.cube_maps {
            put_cstr(&mut out, "cube");
            put_cstr(&mut out, "/textures/environment/cube.dds");
        }
        put_f32s(&mut out, &[1.5; 23]); // lighting
        match self.water {
            Some((elevation, deep, abyss)) => {
                out.push(1);
                put_f32s(&mut out, &[elevation, deep, abyss]);
                put_f32s(&mut out, &[0.25; 20]);
            }
            None => {
                out.push(0);
                put_f32s(&mut out, &[-10_000.0; 3]);
                put_f32s(&mut out, &[0.25; 20]);
            }
        }
        put_cstr(&mut out, "watercube");
        put_cstr(&mut out, "waterramp");
        put_f32s(&mut out, &[2.0; 4]);
        for _ in 0..4 {
            put_f32s(&mut out, &[0.5; 2]);
            put_cstr(&mut out, "wavenormal");
        }
        put_i32(&mut out, len32(self.wave_generators));
        for _ in 0..self.wave_generators {
            put_cstr(&mut out, "wavetexture");
            put_cstr(&mut out, "waveramp");
            put_f32s(&mut out, &[3.0; 17]);
        }
        for colour in 1..=6 {
            put_i32(&mut out, colour * 0x0101_0101);
        }
        if self.version > 56 {
            put_f32s(&mut out, &[0.75]);
        }
        for &(texture, scale) in &self.strata {
            put_cstr(&mut out, texture);
            put_f32s(&mut out, &[scale]);
        }
        let strata_end = out.len();
        for _ in 0..NORMALS {
            put_cstr(&mut out, "/textures/test/normal.dds");
            put_f32s(&mut out, &[8.0]);
        }
        put_i32(&mut out, 0);
        put_i32(&mut out, 0);
        Built {
            bytes: out,
            version_at,
            strata_end,
        }
    }
}

fn parsed(bytes: &[u8]) -> Map {
    let map = Map::parse(bytes);
    assert!(
        map.is_ok(),
        "Map::parse must succeed, got {:?}",
        map.as_ref().err()
    );
    map.expect("parsed: asserted above")
}

/// Pins `Error::BadSignature`: a full-length file whose first word is not
/// `0x1a70614d` is rejected with that word.
#[test]
fn scmap_wrong_signature_reports_the_found_word() {
    let mut buf = Spec::minimal().build().bytes;
    buf[..4].copy_from_slice(&0xdead_beef_u32.to_le_bytes());
    let result = Map::parse(&buf);
    assert!(
        matches!(result, Err(Error::BadSignature { found: 0xdead_beef })),
        "got {:?}",
        result.err()
    );
}

/// Pins `Error::Truncated` for the fixed header: a buffer shorter than the
/// 30-byte fixed header reports needing exactly 30 bytes and the length it
/// has, whether it is empty, carries only the signature, or lacks one byte.
#[test]
fn scmap_header_shorter_than_30_bytes_is_truncated() {
    let full = Spec::minimal().build().bytes;
    for len in [0, 4, 29] {
        let result = Map::parse(&full[..len]);
        assert!(
            matches!(result, Err(Error::Truncated { needed: 30, len: l }) if l == len),
            "{len} bytes: got {:?}",
            result.err()
        );
    }
}

/// Pins `Error::UnsupportedVersion`: an inner version other than 56 or 60 is
/// rejected with the version the file carries; 0, 55, 57, and 61 are
/// rejected.
#[test]
fn scmap_inner_version_other_than_56_or_60_is_unsupported() {
    for found in [0, 55, 57, 61] {
        let built = Spec::minimal().build();
        let mut buf = built.bytes;
        set_i32(&mut buf, built.version_at, found);
        let result = Map::parse(&buf);
        assert!(
            matches!(result, Err(Error::UnsupportedVersion { found: v }) if i64::from(v) == i64::from(found)),
            "version {found}: got {:?}",
            result.err()
        );
    }
}

/// Pins every field of the minimal map: a version-60 file with a 1x1
/// heightmap (so four signed samples, row by row, negatives included) and one
/// stratum. The world size in the header (8 by 4) and the heightmap grid are
/// independent fields, so they are not required to agree; the strata come
/// back as ten slots with the path as stored and nine empty.
#[test]
fn scmap_minimal_map_parses_and_round_trips_its_fields() {
    let spec = Spec::minimal();
    assert_eq!(parsed(&spec.build().bytes), spec.expected());
}

/// Pins the version-56 layout: a version-56 file omits the single unknown
/// float that a version-60 file carries before its strata, and parses to the
/// same fields with `version` 56.
#[test]
fn scmap_version_56_map_parses_like_version_60() {
    let spec = Spec {
        version: 56,
        ..Spec::minimal()
    };
    let map = parsed(&spec.build().bytes);
    assert_eq!(map.version, 56);
    assert_eq!(map, spec.expected());
}

/// Pins the strata slots: the first and the last of the ten layers keep
/// their positions, paths exactly as stored (a leading `/` and backslashes
/// are not normalized by the parser), and scales; the eight between are
/// empty.
#[test]
fn scmap_strata_keep_their_slots_paths_and_scales() {
    let spec = Spec {
        strata: strata(&[
            (0, "/textures/test/a.dds", 4.0),
            (9, "\\Textures\\Test\\B.DDS", 16.0),
        ]),
        ..Spec::minimal()
    };
    let map = parsed(&spec.build().bytes);
    assert_eq!(map.strata.len(), STRATA);
    assert_eq!(map.strata, spec.expected().strata);
    assert_eq!(map.strata[9].texture, "\\Textures\\Test\\B.DDS");
    assert!(map.strata[1..9].iter().all(|s| s.texture.is_empty()));
}

/// Pins the walk to the strata: with three cube maps and two wave
/// generators between the heightmap and the strata, the strata and every
/// other field are the same as with none, so the parser locates the strata
/// by the sections' structure and not by their size.
#[test]
fn scmap_parse_walks_every_section_before_the_strata() {
    let plain = Spec::minimal();
    let busy = Spec {
        cube_maps: 3,
        wave_generators: 2,
        ..Spec::minimal()
    };
    assert_eq!(parsed(&busy.build().bytes), busy.expected());
    assert_eq!(parsed(&busy.build().bytes), parsed(&plain.build().bytes));
}

/// Pins `Error::Truncated` for the heightmap: a header claiming a
/// 65,535-cell square heightmap over a buffer without its samples is
/// truncated and does not try to allocate the samples.
#[test]
fn scmap_heightmap_beyond_the_buffer_is_truncated() {
    let spec = Spec {
        hm_width: 65_535,
        hm_height: 65_535,
        samples: Vec::new(),
        ..Spec::minimal()
    };
    let buf = spec.build().bytes;
    let len = buf.len();
    let result = Map::parse(&buf);
    assert!(
        matches!(result, Err(Error::Truncated { needed, len: l }) if l == len && needed > len),
        "got {:?}",
        result.err()
    );
}

/// Pins `Error::Malformed` for a heightmap dimension: a negative width is
/// not a size.
#[test]
fn scmap_negative_heightmap_dimension_is_malformed() {
    let spec = Spec {
        hm_width: -1,
        ..Spec::minimal()
    };
    let result = Map::parse(&spec.build().bytes);
    assert!(
        matches!(result, Err(Error::Malformed { .. })),
        "got {:?}",
        result.err()
    );
}

/// Pins `Error::Malformed` for the preview: a negative byte length is not a
/// length.
#[test]
fn scmap_negative_preview_length_is_malformed() {
    let mut buf = Spec::minimal().build().bytes;
    set_i32(&mut buf, HEADER_LEN, -1);
    let result = Map::parse(&buf);
    assert!(
        matches!(result, Err(Error::Malformed { .. })),
        "got {:?}",
        result.err()
    );
}

/// Pins `Error::Truncated` for the preview: a length of one million over a
/// buffer that holds eight bytes of it reports exactly the fixed header, the
/// length word, and the claimed bytes as `needed`.
#[test]
fn scmap_preview_beyond_the_buffer_is_truncated() {
    let mut buf = Spec::minimal().build().bytes;
    set_i32(&mut buf, HEADER_LEN, 1_000_000);
    let len = buf.len();
    let result = Map::parse(&buf);
    assert!(
        matches!(result, Err(Error::Truncated { needed, len: l }) if l == len && needed == HEADER_LEN + 4 + 1_000_000),
        "got {:?}",
        result.err()
    );
}

/// Pins totality: every proper prefix of a valid map that ends before the
/// end of the strata is `Truncated` (a string cut before its terminator
/// counts), never a panic and never a success, and reports the length it was
/// given; a prefix shorter than the fixed header needs exactly 30 bytes.
#[test]
fn scmap_every_prefix_before_the_strata_end_is_truncated() {
    let built = Spec {
        cube_maps: 1,
        wave_generators: 1,
        ..Spec::minimal()
    }
    .build();
    for len in 0..built.strata_end {
        let result = Map::parse(&built.bytes[..len]);
        assert!(
            matches!(result, Err(Error::Truncated { needed, len: l })
                if l == len && needed > len && (len >= HEADER_LEN || needed == HEADER_LEN)),
            "prefix of {len} bytes: got {:?}",
            result.err()
        );
    }
}
