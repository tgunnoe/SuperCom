//! Loader for the binary `.scmap` map file: the terrain heightmap, the
//! embedded preview image, the water's presence and elevations, and the
//! terrain texture layers (strata).
//!
//! A `.scmap` is little-endian. It opens with a 30-byte fixed header
//! (signature `0x1a70614d`, major version 2, two constant words, the map's
//! width and height as `f32`, a zero `i32`, a zero `i16`), then the preview
//! image (an `i32` byte length, then that many bytes of a whole `.dds`
//! file), an `i32` inner version (56 or 60), the heightmap, and a long run
//! of sections (terrain shader paths, cube maps, lighting, water and its
//! wave generators, minimap colours) that this crate walks to reach the
//! terrain strata. Decals, props, and the sections after the strata are not
//! read.
//!
//! The layout follows the MIT-licensed reference reader
//! `SCMapImporter.java` at
//! <https://github.com/FAForever/Neroxis-Map-Generator> (commit 10b130836034,
//! `shared/src/main/java/com/faforever/neroxis/importer/SCMapImporter.java`,
//! lines 58-149): the preview at 78 and 446-453, the heightmap at 82-89, the
//! water at 109 and 353-357, the strata at 126-136. The map file is binary; the `_scenario.lua` and
//! `_save.lua` beside it are not read here.
//!
//! [`Map::parse`] walks the file front to back with a checked cursor. Each
//! section between the heightmap and the strata is read or skipped by the
//! size its own fields give, in the order the file stores them: three
//! NUL-terminated shader and sky paths; an `i32` cube map count, then a name
//! and a path (both NUL-terminated) per cube map; the lighting block of 23
//! `f32`; the water block (a presence byte, 23 `f32`, the first three the
//! surface, deep, and abyss elevations and the rest skipped, two paths, four `f32`
//! of wave repeats, then four wave normals each with two `f32` and a path);
//! an `i32` count of wave generators, each two paths and 17 `f32`; six `i32`
//! minimap words; and, in a version-60 file only, one more `f32`. Strings
//! are decoded as ISO-8859-1. Every length is checked against the buffer
//! before it is read or allocated, so a lying count is [`Error::Truncated`]
//! and never an allocation.

use std::fmt;

/// A forward cursor over a byte slice. Every read is checked: a read past the
/// end is [`Error::Truncated`] carrying the length the buffer would need,
/// computed with saturating arithmetic so that no file value overflows it.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn truncated(&self, needed: usize) -> Error {
        Error::Truncated {
            needed,
            len: self.bytes.len(),
        }
    }

    /// The next `n` bytes.
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let needed = self.pos.saturating_add(n);
        let found = self.bytes.get(self.pos..).and_then(|rest| rest.get(..n));
        let slice = found.ok_or_else(|| self.truncated(needed))?;
        self.pos = needed;
        Ok(slice)
    }

    /// Steps over the next `n` bytes.
    fn skip(&mut self, n: usize) -> Result<(), Error> {
        self.take(n).map(drop)
    }

    /// The next `N` bytes as an array.
    fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        let needed = self.pos.saturating_add(N);
        let found = self
            .bytes
            .get(self.pos..)
            .and_then(|rest| rest.first_chunk::<N>())
            .copied();
        let array = found.ok_or_else(|| self.truncated(needed))?;
        self.pos = needed;
        Ok(array)
    }

    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn i32(&mut self) -> Result<i32, Error> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    fn f32(&mut self) -> Result<f32, Error> {
        Ok(f32::from_le_bytes(self.array()?))
    }

    /// A non-negative `i32` read as a count or length; a negative one is
    /// [`Error::Malformed`], naming `what`.
    fn count(&mut self, what: &str) -> Result<u32, Error> {
        let value = self.i32()?;
        u32::try_from(value).map_err(|_| malformed(format!("{what} is {value}, a negative size")))
    }

    /// A NUL-terminated string, decoded as ISO-8859-1 (one `char` per byte).
    /// A string with no terminator before the end of the buffer needs one
    /// more byte than the buffer has.
    fn cstr(&mut self) -> Result<String, Error> {
        let rest = self.bytes.get(self.pos..).unwrap_or_default();
        let Some(end) = rest.iter().position(|&b| b == 0) else {
            return Err(self.truncated(self.bytes.len().saturating_add(1)));
        };
        self.pos = self.pos.saturating_add(end).saturating_add(1);
        Ok(rest.iter().take(end).map(|&b| char::from(b)).collect())
    }
}

/// A count as a `usize`, saturating where `usize` is narrower than 32 bits
/// so that the bounds checks still reject it.
fn to_usize(count: u32) -> usize {
    usize::try_from(count).unwrap_or(usize::MAX)
}

fn malformed(reason: impl Into<String>) -> Error {
    Error::Malformed {
        reason: reason.into(),
    }
}

/// Bytes in the fixed header.
const HEADER_LEN: usize = 30;
/// The first word of every map.
const SIGNATURE: u32 = 0x1a70_614d;
/// The only major version read.
const MAJOR_VERSION: i32 = 2;
/// Terrain albedo strata in every map.
const STRATA: usize = 10;
/// `f32` in the lighting block.
const LIGHTING_FLOATS: usize = 23;
/// `f32` in the water block before its first path.
const WATER_FLOATS: usize = 23;
/// Wave normal slots in the water block.
const WAVE_NORMALS: usize = 4;
/// `f32` in a wave generator after its two paths.
const WAVE_GENERATOR_FLOATS: usize = 17;
/// `i32` minimap colour and contour words.
const MINIMAP_WORDS: usize = 6;

/// The terrain heightmap.
#[derive(Clone, Debug, PartialEq)]
pub struct Heightmap {
    /// Width in cells; the sample grid is one larger in each direction.
    pub width: u32,
    /// Height in cells.
    pub height: u32,
    /// World units per height-sample step, as stored.
    pub scale: f32,
    /// `(width + 1) * (height + 1)` signed 16-bit samples, row by row.
    pub samples: Vec<i16>,
}

/// One terrain texture layer: a path and a tiling scale.
#[derive(Clone, Debug, PartialEq)]
pub struct Stratum {
    /// The texture's path as the map stores it: it may start with `/`, use
    /// `\`, and mix case, and is empty for an unused layer.
    pub texture: String,
    /// The tiling scale stored after the path.
    pub scale: f32,
}

/// The elevations of a map's water surface, in world units. The water's
/// colours, reflection terms, textures, and waves are not read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Water {
    /// Elevation of the water surface.
    pub elevation: f32,
    /// Elevation of the deep-water level.
    pub deep: f32,
    /// Elevation of the abyss level.
    pub abyss: f32,
}

/// A parsed `.scmap` map.
#[derive(Clone, Debug, PartialEq)]
pub struct Map {
    /// Map width in world units, from the fixed header.
    pub width: f32,
    /// Map height in world units, from the fixed header.
    pub height: f32,
    /// The inner version: 56 or 60.
    pub version: u32,
    /// The embedded preview image: a whole `.dds` file.
    pub preview: Vec<u8>,
    /// The terrain heightmap.
    pub heightmap: Heightmap,
    /// The ten terrain texture layers in file order, unused ones with an
    /// empty `texture`.
    pub strata: Vec<Stratum>,
    /// The water elevations; `None` when the map's presence byte is not 1.
    pub water: Option<Water>,
}

impl Map {
    /// Parses the bytes of a `.scmap` file.
    ///
    /// # Errors
    ///
    /// [`Error::Truncated`] when `bytes` is shorter than the 30-byte fixed
    /// header or than a section being read (a string with no terminator
    /// counts); [`Error::BadSignature`] when the first word is not
    /// `0x1a70614d`; [`Error::UnsupportedVersion`] for an inner version other
    /// than 56 or 60 (a negative one is reported as its two's-complement
    /// `u32`); [`Error::Malformed`] for a negative length, count, or
    /// dimension, or a major version other than 2.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let mut file = Reader::new(bytes);
        let mut header = Reader::new(file.take(HEADER_LEN)?);
        let signature = header.u32()?;
        if signature != SIGNATURE {
            return Err(Error::BadSignature { found: signature });
        }
        let major = header.i32()?;
        if major != MAJOR_VERSION {
            return Err(malformed(format!(
                "major version is {major}, not {MAJOR_VERSION}"
            )));
        }
        // Two constant words that carry no information, then the world size.
        header.skip(8)?;
        let width = header.f32()?;
        let height = header.f32()?;

        let preview_len = to_usize(file.count("preview length")?);
        let preview = file.take(preview_len)?.to_vec();
        let version = file.u32()?;
        if version != 56 && version != 60 {
            return Err(Error::UnsupportedVersion { found: version });
        }

        let heightmap = read_heightmap(&mut file)?;
        let water = skip_to_strata(&mut file, version)?;
        let mut strata = Vec::with_capacity(STRATA);
        for _ in 0..STRATA {
            let texture = file.cstr()?;
            let scale = file.f32()?;
            strata.push(Stratum { texture, scale });
        }
        Ok(Self {
            width,
            height,
            version,
            preview,
            heightmap,
            strata,
            water,
        })
    }
}

/// Reads the heightmap section: two `i32` cell counts, an `f32` scale, the
/// samples, and one trailing byte that the reference reader also consumes
/// (importer lines 85-90).
fn read_heightmap(file: &mut Reader<'_>) -> Result<Heightmap, Error> {
    let width = file.count("heightmap width")?;
    let height = file.count("heightmap height")?;
    let scale = file.f32()?;
    let cells = to_usize(width)
        .saturating_add(1)
        .saturating_mul(to_usize(height).saturating_add(1));
    let raw = file.take(cells.saturating_mul(2))?;
    // `raw` is exactly `2 * cells` bytes, so every chunk is a whole sample.
    let samples = raw
        .chunks_exact(2)
        .filter_map(|pair| pair.first_chunk().copied().map(i16::from_le_bytes))
        .collect();
    file.skip(1)?;
    Ok(Heightmap {
        width,
        height,
        scale,
        samples,
    })
}

/// Walks the sections between the heightmap and the strata, in file order
/// (importer lines 94-127), and leaves the cursor at the first stratum.
/// Returns the water elevations read on the way.
fn skip_to_strata(file: &mut Reader<'_>, version: u32) -> Result<Option<Water>, Error> {
    // Shader, background, and sky cube paths.
    for _ in 0..3 {
        file.cstr()?;
    }
    for _ in 0..file.count("cube map count")? {
        file.cstr()?; // name
        file.cstr()?; // path
    }
    file.skip(LIGHTING_FLOATS * 4)?;
    // Water: a presence byte and its numbers, two paths, the wave normals.
    // The block is taken whole so a short buffer errors as one read, and the
    // first three numbers are the elevations (importer lines 354-357); the
    // rest of the block is never read.
    let mut block = Reader::new(file.take(1 + WATER_FLOATS * 4)?);
    let [present] = block.array()?;
    let (elevation, deep, abyss) = (block.f32()?, block.f32()?, block.f32()?);
    let water = (present == 1).then_some(Water {
        elevation,
        deep,
        abyss,
    });
    file.cstr()?;
    file.cstr()?;
    file.skip(WAVE_NORMALS * 4)?;
    for _ in 0..WAVE_NORMALS {
        file.skip(2 * 4)?;
        file.cstr()?;
    }
    for _ in 0..file.count("wave generator count")? {
        file.cstr()?;
        file.cstr()?;
        file.skip(WAVE_GENERATOR_FLOATS * 4)?;
    }
    file.skip(MINIMAP_WORDS * 4)?;
    if version > 56 {
        file.skip(4)?;
    }
    Ok(water)
}

/// Why a map could not be parsed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// The file does not start with the map signature.
    BadSignature {
        /// The first four bytes as a little-endian word.
        found: u32,
    },
    /// The inner version is neither 56 nor 60.
    UnsupportedVersion {
        /// The version number the file carries.
        found: u32,
    },
    /// The buffer ends before a section being read.
    Truncated {
        /// The length the buffer would need to hold what was being read.
        needed: usize,
        /// The length of the buffer.
        len: usize,
    },
    /// A length, count, dimension, or major version is impossible.
    Malformed {
        /// What was impossible, for the reader of the message.
        reason: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadSignature { found } => write!(f, "bad signature {found:#010x}"),
            Self::UnsupportedVersion { found } => write!(f, "unsupported version {found}"),
            Self::Truncated { needed, len } => {
                write!(f, "truncated: needs {needed} bytes, has {len}")
            }
            Self::Malformed { reason } => write!(f, "malformed: {reason}"),
        }
    }
}

impl std::error::Error for Error {}
