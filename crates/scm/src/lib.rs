//! Loader for `.scm` meshes and `.sca` animations: the binary formats of the
//! installed game's models.
//!
//! Both formats are little-endian. A mesh (`MODL`, version 5) is a 48-byte
//! header followed by tables of bones, vertices, and indices placed by file
//! offsets; an animation (`ANIM`, version 5) is a 36-byte header followed by
//! bone names, bone links, a root pose, and fixed-size frames.
//!
//! Both parsers read a byte slice and return typed errors; no input makes
//! them panic. Every offset and count a header places is checked against the
//! buffer before the table it names is read or allocated, so a header that
//! lies about a size costs an [`Error::Truncated`], not memory.
//!
//! Layouts follow the pinned reading of the Blender importer at
//! <https://github.com/Exotic-Retard/SupCom_Import_Export_Blender> (commit
//! d5cd48b6f0c3, `supcom-importer.py`): the mesh header at line 328, the bone
//! record at 208, the vertex at 281, the triangle indices at 412; the
//! animation header at 625, the frame header at 496, the pose at 502, and
//! the root pose at 662. That importer carries no licence, so it is cited
//! and not copied.

use std::fmt;

/// A forward cursor over a byte slice. Every read is checked: a read past the
/// end is [`Error::Truncated`] carrying the length the buffer would need,
/// computed with saturating arithmetic so that no header value overflows it.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /// A cursor at `pos`, which may lie beyond the end: the first read then
    /// reports the truncation.
    fn at(bytes: &'a [u8], pos: usize) -> Self {
        Self { bytes, pos }
    }

    /// The next `n` bytes.
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let needed = self.pos.saturating_add(n);
        let found = self.bytes.get(self.pos..).and_then(|rest| rest.get(..n));
        match found {
            Some(slice) => {
                self.pos = needed;
                Ok(slice)
            }
            None => Err(Error::Truncated {
                needed,
                len: self.bytes.len(),
            }),
        }
    }

    /// The next `N` bytes as an array.
    fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        let needed = self.pos.saturating_add(N);
        let found = self
            .bytes
            .get(self.pos..)
            .and_then(|rest| rest.first_chunk::<N>())
            .copied();
        match found {
            Some(array) => {
                self.pos = needed;
                Ok(array)
            }
            None => Err(Error::Truncated {
                needed,
                len: self.bytes.len(),
            }),
        }
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

    fn floats<const N: usize>(&mut self) -> Result<[f32; N], Error> {
        let mut out = [0.0; N];
        for slot in &mut out {
            *slot = self.f32()?;
        }
        Ok(out)
    }

    /// A NUL-terminated string of at most `max_len` bytes, decoded as
    /// ISO-8859-1 (one `char` per byte). A string with no terminator before
    /// the end of the buffer needs one more byte than the buffer has; one
    /// with no terminator within `max_len` bytes is [`Error::Malformed`].
    fn cstr(&mut self, max_len: usize) -> Result<String, Error> {
        let len = self.bytes.len();
        let rest = self.bytes.get(self.pos..).unwrap_or_default();
        let window = rest.get(..max_len.saturating_add(1)).unwrap_or(rest);
        let Some(end) = window.iter().position(|&b| b == 0) else {
            if window.len() < rest.len() {
                return Err(malformed(format!("a name is longer than {max_len} bytes")));
            }
            return Err(Error::Truncated {
                needed: len.saturating_add(1),
                len,
            });
        };
        self.pos = self.pos.saturating_add(end).saturating_add(1);
        Ok(window.iter().take(end).map(|&b| char::from(b)).collect())
    }
}

/// The longest bone name read. The installed names are far shorter (34
/// bytes at most). Every bone record carries its own name offset, so many
/// records may point at one string and each read decodes its own copy; the
/// bound keeps that memory linear in the file (at most about 1024 bytes of
/// name per 108-byte record) rather than below the file's size. A budget on
/// the decoded bytes is the remaining hardening, tracked in the repository's
/// issues.
const MAX_NAME_LEN: usize = 1024;
/// The only format version either parser reads.
const VERSION: u32 = 5;
const MESH_HEADER_LEN: usize = 48;
const ANIM_HEADER_LEN: usize = 36;
const BONE_LEN: usize = 108;
const VERTEX_LEN: usize = 68;
/// Bytes of one pose: seven `f32`.
const POSE_LEN: usize = 28;
/// Bytes of a frame before its poses: the key time and the flags word.
const FRAME_HEAD_LEN: usize = 8;

/// The byte length of a table of `count` records of `size` bytes, saturating
/// so that an absurd count is reported as a truncation, never an overflow.
fn table_len(count: usize, size: usize) -> usize {
    count.saturating_mul(size)
}

/// Decodes one 108-byte bone record. `bytes` is the whole file, in which the
/// record's name offset points; `total` is the number of bones, the bound on
/// a parent index.
fn read_bone(bytes: &[u8], record: &[u8], total: usize) -> Result<Bone, Error> {
    let mut r = Reader::at(record, 0);
    let mut rest_pose_inverse = [[0.0; 4]; 4];
    for row in &mut rest_pose_inverse {
        *row = r.floats()?;
    }
    let position = r.floats()?;
    let rotation = r.floats()?;
    let name_offset = r.i32()?;
    let parent = r.i32()?;
    let name_offset = usize::try_from(name_offset)
        .map_err(|_| malformed(format!("bone name offset {name_offset} is negative")))?;
    let name = Reader::at(bytes, name_offset).cstr(MAX_NAME_LEN)?;
    let parent = match usize::try_from(parent) {
        Ok(index) if index < total => Some(index),
        _ if parent == -1 => None,
        _ => {
            return Err(malformed(format!(
                "bone parent {parent} names no bone of {total}"
            )));
        }
    };
    Ok(Bone {
        name,
        parent,
        position,
        rotation,
        rest_pose_inverse,
    })
}

/// Decodes one 68-byte vertex record.
fn read_vertex(record: &[u8]) -> Result<Vertex, Error> {
    let mut r = Reader::at(record, 0);
    Ok(Vertex {
        position: r.floats()?,
        tangent: r.floats()?,
        normal: r.floats()?,
        binormal: r.floats()?,
        uv0: r.floats()?,
        uv1: r.floats()?,
        bone_indices: r.array()?,
    })
}

/// Decodes one pose: three position floats, then four rotation floats.
fn read_pose(r: &mut Reader<'_>) -> Result<Pose, Error> {
    Ok(Pose {
        position: r.floats()?,
        rotation: r.floats()?,
    })
}

/// Decodes one frame of `bone_count` poses.
fn read_frame(record: &[u8], bone_count: usize) -> Result<Frame, Error> {
    let mut r = Reader::at(record, 0);
    let time = r.f32()?;
    let _flags = r.i32()?;
    let poses = (0..bone_count)
        .map(|_| read_pose(&mut r))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Frame { time, poses })
}

/// A header word as a `usize`, saturating where `usize` is narrower than 32
/// bits so the bounds checks still reject it.
fn to_usize(word: u32) -> usize {
    usize::try_from(word).unwrap_or(usize::MAX)
}

fn malformed(reason: impl Into<String>) -> Error {
    Error::Malformed {
        reason: reason.into(),
    }
}

/// One bone of a mesh skeleton.
///
/// On disk a bone is a 108-byte record (importer line 208): the rest-pose
/// inverse as sixteen `f32` in row-major order, the position as three `f32`,
/// the rotation as four `f32`, then four `i32`: the file offset of the bone's
/// NUL-terminated name, the parent index (-1 for a root), and two reserved
/// words that are not read.
#[derive(Clone, Debug, PartialEq)]
pub struct Bone {
    /// The bone's name: the NUL-terminated string at the record's name
    /// offset, decoded as ISO-8859-1 (one `char` per byte).
    pub name: String,
    /// Index of the parent bone, or `None` for a root (stored as -1).
    pub parent: Option<usize>,
    /// Rest position of the bone.
    pub position: [f32; 3],
    /// Rest rotation of the bone, as a quaternion in file order.
    pub rotation: [f32; 4],
    /// The rest-pose inverse matrix as stored: row-major, not transposed.
    pub rest_pose_inverse: [[f32; 4]; 4],
}

/// One vertex of a mesh.
///
/// On disk a vertex is a 68-byte record (importer line 281): sixteen `f32`
/// (position, tangent, normal, binormal, two texture coordinate pairs) and
/// four `u8` bone indices. The indices are stored as found; the parser does
/// not check them against the skeleton.
#[derive(Clone, Debug, PartialEq)]
pub struct Vertex {
    /// Position.
    pub position: [f32; 3],
    /// Tangent.
    pub tangent: [f32; 3],
    /// Normal.
    pub normal: [f32; 3],
    /// Binormal.
    pub binormal: [f32; 3],
    /// First texture coordinate set.
    pub uv0: [f32; 2],
    /// Second texture coordinate set.
    pub uv1: [f32; 2],
    /// The four bone indices that skin the vertex.
    pub bone_indices: [u8; 4],
}

/// A parsed `.scm` mesh.
///
/// The file opens with a 48-byte header (importer line 328): the magic
/// `MODL`, then eleven little-endian `u32`: the version, the bone table
/// offset, the bone count, the vertex table offset, an extra-vertex offset,
/// the vertex count, the index table offset, the index count, an info offset,
/// an info count, and the total bone count. Offsets are from the start of the
/// file and place each table, so tables need not be adjacent. The bone
/// count, the extra-vertex offset, and the info words are not read.
/// Triangle indices are `u16` values (importer line 412); the parser checks
/// that their count is a multiple of three but not that each is below the
/// vertex count.
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    /// The skeleton: `total_bone_count` records from the header.
    pub bones: Vec<Bone>,
    /// The vertex table.
    pub vertices: Vec<Vertex>,
    /// The index table: `index_count` values, three to a triangle.
    pub indices: Vec<u16>,
}

impl Mesh {
    /// Parses the bytes of a `.scm` file.
    ///
    /// # Errors
    ///
    /// [`Error::BadMagic`] when the file does not start with `MODL`;
    /// [`Error::UnsupportedVersion`] for any version but 5;
    /// [`Error::Truncated`] when the buffer is shorter than the 48-byte
    /// header or than a table the header places, or when a bone's name lies
    /// past the end; [`Error::Malformed`] when the index count is not a
    /// multiple of three, a bone's name offset is negative or its name is
    /// longer than 1024 bytes, or a parent index names no bone.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let Some(head) = bytes.get(..MESH_HEADER_LEN) else {
            return Err(Error::Truncated {
                needed: MESH_HEADER_LEN,
                len: bytes.len(),
            });
        };
        let mut header = Reader::at(head, 0);
        let magic = header.array::<4>()?;
        if magic != *b"MODL" {
            return Err(Error::BadMagic { found: magic });
        }
        let version = header.u32()?;
        if version != VERSION {
            return Err(Error::UnsupportedVersion { found: version });
        }
        let bone_offset = to_usize(header.u32()?);
        let _bone_count = header.u32()?;
        let vertex_offset = to_usize(header.u32()?);
        let _extra_vertex_offset = header.u32()?;
        let vertex_count = to_usize(header.u32()?);
        let index_offset = to_usize(header.u32()?);
        let index_count = to_usize(header.u32()?);
        let _info_offset = header.u32()?;
        let _info_count = header.u32()?;
        let total_bone_count = to_usize(header.u32()?);

        if index_count % 3 != 0 {
            return Err(malformed(format!(
                "index count {index_count} is not a multiple of 3"
            )));
        }

        let bone_table =
            Reader::at(bytes, bone_offset).take(table_len(total_bone_count, BONE_LEN))?;
        let vertex_table =
            Reader::at(bytes, vertex_offset).take(table_len(vertex_count, VERTEX_LEN))?;
        let index_table = Reader::at(bytes, index_offset).take(table_len(index_count, 2))?;

        let bones = bone_table
            .chunks_exact(BONE_LEN)
            .map(|record| read_bone(bytes, record, total_bone_count))
            .collect::<Result<Vec<_>, _>>()?;
        let vertices = vertex_table
            .chunks_exact(VERTEX_LEN)
            .map(read_vertex)
            .collect::<Result<Vec<_>, _>>()?;
        let indices = index_table
            .chunks_exact(2)
            .map(|pair| Reader::at(pair, 0).array::<2>().map(u16::from_le_bytes))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            bones,
            vertices,
            indices,
        })
    }
}

/// A bone's position and rotation at one instant of an animation: seven
/// `f32` on disk (importer lines 502 and 662), three of position then four of
/// rotation.
#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    /// Position.
    pub position: [f32; 3],
    /// Rotation, as a quaternion in file order.
    pub rotation: [f32; 4],
}

/// One key frame of an animation: a key time and one pose per bone.
///
/// On disk a frame is `8 + 28 * bone_count` bytes (importer line 496): the
/// key time as an `f32`, a flags `i32` that is not read, then one pose per
/// bone.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// The key time, in seconds.
    pub time: f32,
    /// One pose per bone, in the order of [`Animation::bone_names`].
    pub poses: Vec<Pose>,
}

/// A parsed `.sca` animation.
///
/// The file opens with a 36-byte header (importer line 625): the magic
/// `ANIM`, then eight little-endian words: the version (`i32`), the frame
/// count (`u32`), the duration (`f32`), the bone count, the offset of the
/// bone names, the offset of the bone links, the offset of the animation
/// data, and the frame size (each `u32`). The names are NUL-terminated
/// strings one after another, the links are one `i32` per bone, and the data
/// is the root pose followed by the frames (importer line 662).
#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    /// The frame count from the header; equals `frames.len()`.
    pub frame_count: u32,
    /// The duration in seconds, from the header.
    pub duration: f32,
    /// The animated bones' names, in file order.
    pub bone_names: Vec<String>,
    /// The bone links as stored: one `i32` per bone, the parent index.
    pub bone_links: Vec<i32>,
    /// The single pose that precedes the frames. It is the animation's root
    /// pose and is not a frame.
    pub root: Pose,
    /// The key frames, in file order.
    pub frames: Vec<Frame>,
}

impl Animation {
    /// Parses the bytes of a `.sca` file.
    ///
    /// # Errors
    ///
    /// [`Error::BadMagic`] when the file does not start with `ANIM`;
    /// [`Error::UnsupportedVersion`] for any version but 5;
    /// [`Error::Truncated`] when the buffer is shorter than the 36-byte
    /// header or than the data the header places; [`Error::Malformed`] when
    /// `frame_size` is not `8 + 28 * bone_count` or a bone name is longer
    /// than 1024 bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let Some(head) = bytes.get(..ANIM_HEADER_LEN) else {
            return Err(Error::Truncated {
                needed: ANIM_HEADER_LEN,
                len: bytes.len(),
            });
        };
        let mut header = Reader::at(head, 0);
        let magic = header.array::<4>()?;
        if magic != *b"ANIM" {
            return Err(Error::BadMagic { found: magic });
        }
        let version = header.u32()?;
        if version != VERSION {
            return Err(Error::UnsupportedVersion { found: version });
        }
        let frame_count_word = header.u32()?;
        let duration = header.f32()?;
        let bone_count = to_usize(header.u32()?);
        let names_offset = to_usize(header.u32()?);
        let links_offset = to_usize(header.u32()?);
        let data_offset = to_usize(header.u32()?);
        let frame_size = to_usize(header.u32()?);

        if frame_size
            != POSE_LEN
                .saturating_mul(bone_count)
                .saturating_add(FRAME_HEAD_LEN)
        {
            return Err(malformed(format!(
                "frame size {frame_size} is not {FRAME_HEAD_LEN} + {POSE_LEN} * {bone_count} bones"
            )));
        }

        // The links table is checked first: it bounds the bone count by the
        // buffer before anything is sized by it.
        let link_table = Reader::at(bytes, links_offset).take(table_len(bone_count, 4))?;
        let bone_links = link_table
            .chunks_exact(4)
            .map(|word| Reader::at(word, 0).i32())
            .collect::<Result<Vec<_>, _>>()?;
        let mut names = Reader::at(bytes, names_offset);
        let bone_names = (0..bone_count)
            .map(|_| names.cstr(MAX_NAME_LEN))
            .collect::<Result<Vec<_>, _>>()?;

        let frame_count = to_usize(frame_count_word);
        let data_len = table_len(frame_count, frame_size).saturating_add(POSE_LEN);
        let data = Reader::at(bytes, data_offset).take(data_len)?;
        let Some((root_bytes, frame_bytes)) = data.split_at_checked(POSE_LEN) else {
            return Err(Error::Truncated {
                needed: data_offset.saturating_add(POSE_LEN),
                len: bytes.len(),
            });
        };
        let root = read_pose(&mut Reader::at(root_bytes, 0))?;
        // `frame_size` equals `8 + 28 * bone_count` here, so it is not zero.
        let frames = frame_bytes
            .chunks_exact(frame_size)
            .map(|record| read_frame(record, bone_count))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            frame_count: frame_count_word,
            duration,
            bone_names,
            bone_links,
            root,
            frames,
        })
    }
}

/// Why a mesh or animation could not be parsed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// The file does not start with the expected magic.
    BadMagic {
        /// The four bytes the file starts with.
        found: [u8; 4],
    },
    /// The format version is not the one this crate reads.
    UnsupportedVersion {
        /// The version number the header carries.
        found: u32,
    },
    /// The buffer ends before a field or table the header places.
    Truncated {
        /// The length the buffer would need to hold what was being read.
        needed: usize,
        /// The length of the buffer.
        len: usize,
    },
    /// The header and the data it describes disagree.
    Malformed {
        /// What disagreed, for the reader of the message.
        reason: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic { found } => write!(f, "bad magic {found:?}"),
            Self::UnsupportedVersion { found } => write!(f, "unsupported version {found}"),
            Self::Truncated { needed, len } => {
                write!(f, "truncated: needs {needed} bytes, has {len}")
            }
            Self::Malformed { reason } => write!(f, "malformed: {reason}"),
        }
    }
}

impl std::error::Error for Error {}
