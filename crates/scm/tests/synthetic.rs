//! Hermetic behaviour of `supercom-scm` over buffers built in code. No game
//! is needed and no fixture file is committed: every mesh and animation here
//! is assembled from literals, byte by byte, following the layouts in the
//! crate documentation.
//!
//! These pin what the installed game cannot: the exact error for each way a
//! file can be wrong, and the meaning of every field of a minimal file. The
//! crate is a skeleton, so each test below fails today on
//! `Error::Unimplemented` and passes only when the parser exists.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use supercom_scm::{Animation, Bone, Error, Frame, Mesh, Pose, Vertex};

const MESH_HEADER_LEN: usize = 48;
const BONE_LEN: usize = 108;
const VERTEX_LEN: usize = 68;
const ANIM_HEADER_LEN: usize = 36;
const POSE_LEN: usize = 28;

// Byte offsets of the mesh header's fields.
const VERSION_AT: usize = 4;
const BONE_COUNT_AT: usize = 12;
const EXTRA_VERTEX_OFFSET_AT: usize = 20;
const VERTEX_COUNT_AT: usize = 24;

// Byte offsets of the animation header's fields.
const FRAME_COUNT_AT: usize = 8;
const FRAME_SIZE_AT: usize = 32;

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_f32s(out: &mut Vec<u8>, values: &[f32]) {
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
}

fn len32(len: usize) -> u32 {
    u32::try_from(len).expect("test buffers are far below 4 GiB")
}

/// Overwrites the little-endian `u32` at `at`.
fn set_u32(buf: &mut [u8], at: usize, value: u32) {
    buf[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// A bone of a synthetic mesh: the file's fields, before packing.
struct BoneSpec {
    name: &'static str,
    parent: i32,
    position: [f32; 3],
    rotation: [f32; 4],
    rest: [[f32; 4]; 4],
}

/// A root bone whose every stored float is distinct, so a transposed matrix
/// or a swapped field is visible.
fn root_bone() -> BoneSpec {
    BoneSpec {
        name: "root",
        parent: -1,
        position: [0.5, 1.5, 2.5],
        rotation: [0.0, 0.0, 0.0, 1.0],
        rest: [
            [1.0, 2.0, 3.0, 4.0],
            [5.0, 6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0, 12.0],
            [13.0, 14.0, 15.0, 16.0],
        ],
    }
}

fn child_bone(parent: i32) -> BoneSpec {
    BoneSpec {
        name: "arm",
        parent,
        position: [-0.5, -1.5, -2.5],
        rotation: [0.5, 0.5, 0.5, 0.5],
        rest: [
            [-1.0, -2.0, -3.0, -4.0],
            [-5.0, -6.0, -7.0, -8.0],
            [-9.0, -10.0, -11.0, -12.0],
            [-13.0, -14.0, -15.0, -16.0],
        ],
    }
}

/// The sixteen floats of vertex `seed`, in file order: position, normal,
/// tangent, binormal, uv0, uv1. Distinct across seeds and positions.
fn vertex_floats(seed: u8) -> [f32; 16] {
    std::array::from_fn(|k| f32::from(seed) * 16.0 + f32::from(u8::try_from(k).unwrap_or(0)) * 0.5)
}

fn vertex_bones(seed: u8) -> [u8; 4] {
    [seed, seed + 1, 0, 0]
}

fn expected_vertex(seed: u8) -> Vertex {
    let f = vertex_floats(seed);
    Vertex {
        position: [f[0], f[1], f[2]],
        normal: [f[3], f[4], f[5]],
        tangent: [f[6], f[7], f[8]],
        binormal: [f[9], f[10], f[11]],
        uv0: [f[12], f[13]],
        uv1: [f[14], f[15]],
        bone_indices: vertex_bones(seed),
    }
}

fn expected_bone(spec: &BoneSpec) -> Bone {
    Bone {
        name: spec.name.to_owned(),
        parent: usize::try_from(spec.parent).ok(),
        position: spec.position,
        rotation: spec.rotation,
        rest_pose_inverse: spec.rest,
    }
}

/// The bytes of a version-5 mesh: header, the bone names, then the bone,
/// vertex, and index tables, `gap` filler bytes before each table. The
/// header counts are the real ones; tests patch them to make a bad file.
fn mesh_bytes(bones: &[BoneSpec], vertices: u8, indices: &[u16], gap: usize) -> Vec<u8> {
    let mut names = Vec::new();
    let mut name_offsets = Vec::new();
    for bone in bones {
        name_offsets.push(MESH_HEADER_LEN + names.len());
        names.extend_from_slice(bone.name.as_bytes());
        names.push(0);
    }
    let bone_offset = MESH_HEADER_LEN + names.len() + gap;
    let vertex_offset = bone_offset + bones.len() * BONE_LEN + gap;
    let index_offset = vertex_offset + usize::from(vertices) * VERTEX_LEN + gap;
    let end = index_offset + indices.len() * 2;

    let mut out = b"MODL".to_vec();
    for field in [
        5,
        len32(bone_offset),
        len32(bones.len()),
        len32(vertex_offset),
        len32(vertex_offset),
        u32::from(vertices),
        len32(index_offset),
        len32(indices.len()),
        len32(end),
        0,
        len32(bones.len()),
    ] {
        put_u32(&mut out, field);
    }
    out.extend_from_slice(&names);
    out.resize(bone_offset, 0xEE);
    for (bone, name_offset) in bones.iter().zip(name_offsets) {
        for row in &bone.rest {
            put_f32s(&mut out, row);
        }
        put_f32s(&mut out, &bone.position);
        put_f32s(&mut out, &bone.rotation);
        put_i32(&mut out, i32::try_from(name_offset).expect("small offset"));
        put_i32(&mut out, bone.parent);
        put_i32(&mut out, 0);
        put_i32(&mut out, 0);
    }
    out.resize(vertex_offset, 0xEE);
    for seed in 0..vertices {
        put_f32s(&mut out, &vertex_floats(seed));
        out.extend_from_slice(&vertex_bones(seed));
    }
    out.resize(index_offset, 0xEE);
    for index in indices {
        out.extend_from_slice(&index.to_le_bytes());
    }
    out
}

/// The smallest mesh the format allows: one bone, three vertices, one
/// triangle.
fn minimal_mesh() -> Vec<u8> {
    mesh_bytes(&[root_bone()], 3, &[0, 1, 2], 0)
}

fn parsed_mesh(bytes: &[u8]) -> Mesh {
    let mesh = Mesh::parse(bytes);
    assert!(
        mesh.is_ok(),
        "Mesh::parse must succeed, got {:?}",
        mesh.as_ref().err()
    );
    mesh.expect("parsed: asserted above")
}

/// Pins `Error::BadMagic` for meshes: a full-length file that does not start
/// with `MODL` is rejected with the four bytes it does start with, including
/// an animation handed to the mesh parser.
#[test]
fn scm_mesh_wrong_magic_reports_the_found_bytes() {
    let mut buf = minimal_mesh();
    buf[..4].copy_from_slice(b"XXXX");
    assert!(
        matches!(Mesh::parse(&buf), Err(Error::BadMagic { found }) if found == *b"XXXX"),
        "got {:?}",
        Mesh::parse(&buf).err()
    );

    let anim = anim_bytes(&["root"], &[-1], &ROOT_POSE, &[], 0.0, 0);
    assert!(
        matches!(Mesh::parse(&anim), Err(Error::BadMagic { found }) if found == *b"ANIM"),
        "got {:?}",
        Mesh::parse(&anim).err()
    );
}

/// Pins `Error::Truncated` for a mesh header: a buffer shorter than the
/// 48-byte header reports needing exactly 48 bytes and the length it has,
/// whether it is empty, carries only the magic, or lacks one byte.
#[test]
fn scm_mesh_header_shorter_than_48_bytes_is_truncated() {
    let full = minimal_mesh();
    for len in [0, 4, 47] {
        let result = Mesh::parse(&full[..len]);
        assert!(
            matches!(result, Err(Error::Truncated { needed: 48, len: l }) if l == len),
            "{len} bytes: got {:?}",
            result.err()
        );
    }
}

/// Pins `Error::Truncated` for a table: a header whose vertex table
/// (`vertex_offset + vertex_count * 68`) ends beyond the buffer is truncated,
/// and `needed` is exactly that end.
#[test]
fn scm_mesh_vertex_table_past_the_buffer_is_truncated() {
    let mut buf = minimal_mesh();
    let vertex_offset = MESH_HEADER_LEN + "root\0".len() + BONE_LEN;
    set_u32(&mut buf, VERTEX_COUNT_AT, 1000);
    let needed = vertex_offset + 1000 * VERTEX_LEN;
    let len = buf.len();
    assert!(
        matches!(Mesh::parse(&buf), Err(Error::Truncated { needed: n, len: l }) if n == needed && l == len),
        "expected Truncated {{ needed: {needed}, len: {len} }}, got {:?}",
        Mesh::parse(&buf).err()
    );
}

/// Pins `Error::UnsupportedVersion` for meshes: versions 3, 4, and 6 are
/// rejected with the version the header carries; only 5 is read.
#[test]
fn scm_mesh_version_other_than_5_is_unsupported() {
    for found in [3, 4, 6] {
        let mut buf = minimal_mesh();
        set_u32(&mut buf, VERSION_AT, found);
        let result = Mesh::parse(&buf);
        assert!(
            matches!(result, Err(Error::UnsupportedVersion { found: v }) if v == found),
            "version {found}: got {:?}",
            result.err()
        );
    }
}

/// Pins every field of the minimal mesh (one bone, three vertices, one
/// triangle): bone name, parent, position, rotation, and the rest-pose
/// inverse as stored row-major; each vertex's sixteen floats and four bone
/// indices; the index table.
#[test]
fn scm_minimal_mesh_parses_and_round_trips_its_fields() {
    let mesh = parsed_mesh(&minimal_mesh());
    let expected = Mesh {
        bones: vec![expected_bone(&root_bone())],
        vertices: (0..3).map(expected_vertex).collect(),
        indices: vec![0, 1, 2],
    };
    assert_eq!(mesh, expected);
}

/// Pins the parent link: a stored parent of -1 is `None` and a stored index
/// is `Some(index)`, so the child of bone 0 reports `Some(0)`.
#[test]
fn scm_mesh_parent_minus_one_is_none_and_an_index_is_some() {
    let bones = [root_bone(), child_bone(0)];
    let mesh = parsed_mesh(&mesh_bytes(&bones, 3, &[0, 1, 2], 0));
    let parents: Vec<Option<usize>> = mesh.bones.iter().map(|bone| bone.parent).collect();
    assert_eq!(parents, vec![None, Some(0)]);
    assert_eq!(mesh.bones[1].name, "arm");
}

/// Pins the bone table's length: `bones` holds `total_bone_count` records,
/// not `bone_count` (the two differ in 389 installed meshes), so a header
/// with `bone_count` 1 and `total_bone_count` 2 yields two bones.
#[test]
fn scm_mesh_bones_come_from_total_bone_count() {
    let bones = [root_bone(), child_bone(0)];
    let mut buf = mesh_bytes(&bones, 3, &[0, 1, 2], 0);
    set_u32(&mut buf, BONE_COUNT_AT, 1);
    let mesh = parsed_mesh(&buf);
    assert_eq!(mesh.bones.len(), 2);
}

/// Pins that `extra_vertex_offset` is ignored: filling the field with
/// garbage changes nothing in the parse.
#[test]
fn scm_mesh_ignores_the_extra_vertex_offset() {
    let clean = parsed_mesh(&minimal_mesh());
    let mut buf = minimal_mesh();
    set_u32(&mut buf, EXTRA_VERTEX_OFFSET_AT, u32::MAX);
    assert_eq!(parsed_mesh(&buf), clean);
}

/// Pins that the header's offsets place the tables: with filler bytes before
/// each table, so nothing is adjacent, the parse equals the compact one.
#[test]
fn scm_mesh_honours_table_offsets_over_adjacency() {
    let compact = parsed_mesh(&minimal_mesh());
    let spaced = parsed_mesh(&mesh_bytes(&[root_bone()], 3, &[0, 1, 2], 7));
    assert_eq!(spaced, compact);
}

/// Pins `Error::Malformed` for an index table that is not whole triangles:
/// `index_count` must be a multiple of 3 (it is in all 1,938 installed
/// meshes).
#[test]
fn scm_mesh_index_count_not_a_multiple_of_three_is_malformed() {
    let buf = mesh_bytes(&[root_bone()], 3, &[0, 1, 2, 0], 0);
    let result = Mesh::parse(&buf);
    assert!(
        matches!(result, Err(Error::Malformed { .. })),
        "got {:?}",
        result.err()
    );
}

/// Pins `Error::Malformed` for a parent index that names no bone: a stored
/// parent of 5 in a one-bone mesh is neither -1 nor below
/// `total_bone_count`.
#[test]
fn scm_mesh_parent_beyond_the_bone_table_is_malformed() {
    let mut orphan = root_bone();
    orphan.parent = 5;
    let result = Mesh::parse(&mesh_bytes(&[orphan], 3, &[0, 1, 2], 0));
    assert!(
        matches!(result, Err(Error::Malformed { .. })),
        "got {:?}",
        result.err()
    );
}

/// Pins totality: every proper prefix of a valid mesh is `Truncated`, never a
/// panic and never a success, and reports the length it was given.
#[test]
fn scm_mesh_every_proper_prefix_is_truncated() {
    let full = minimal_mesh();
    for len in 0..full.len() {
        let result = Mesh::parse(&full[..len]);
        assert!(
            matches!(result, Err(Error::Truncated { needed, len: l }) if l == len && needed > len),
            "prefix of {len} bytes: got {:?}",
            result.err()
        );
    }
}

/// The root pose of the synthetic animations: seven distinct floats.
const ROOT_POSE: [f32; 7] = [10.0, 11.0, 12.0, 0.0, 0.0, 0.0, 1.0];

/// The bytes of a version-5 animation: header, bone names, `gap` filler,
/// bone links, `gap` filler, the root pose, then each frame (key time,
/// flags, one pose per bone). A pose is position then rotation.
fn anim_bytes(
    names: &[&str],
    links: &[i32],
    root: &[f32; 7],
    frames: &[(f32, Vec<[f32; 7]>)],
    duration: f32,
    gap: usize,
) -> Vec<u8> {
    let mut names_block = Vec::new();
    for name in names {
        names_block.extend_from_slice(name.as_bytes());
        names_block.push(0);
    }
    let names_offset = ANIM_HEADER_LEN;
    let links_offset = names_offset + names_block.len() + gap;
    let data_offset = links_offset + links.len() * 4 + gap;

    let mut out = b"ANIM".to_vec();
    put_i32(&mut out, 5);
    put_u32(&mut out, len32(frames.len()));
    put_f32s(&mut out, &[duration]);
    put_u32(&mut out, len32(names.len()));
    put_u32(&mut out, len32(names_offset));
    put_u32(&mut out, len32(links_offset));
    put_u32(&mut out, len32(data_offset));
    put_u32(&mut out, len32(8 + POSE_LEN * names.len()));
    out.extend_from_slice(&names_block);
    out.resize(links_offset, 0xEE);
    for link in links {
        put_i32(&mut out, *link);
    }
    out.resize(data_offset, 0xEE);
    put_f32s(&mut out, root);
    for (time, poses) in frames {
        put_f32s(&mut out, &[*time]);
        put_i32(&mut out, 0);
        for pose in poses {
            put_f32s(&mut out, pose);
        }
    }
    out
}

fn pose(values: &[f32; 7]) -> Pose {
    Pose {
        position: [values[0], values[1], values[2]],
        rotation: [values[3], values[4], values[5], values[6]],
    }
}

/// The smallest animation the format allows: one bone, one frame.
fn minimal_anim() -> Vec<u8> {
    let frame = (0.5, vec![[1.0, 2.0, 3.0, 0.0, 0.0, 0.0, 1.0]]);
    anim_bytes(&["root"], &[-1], &ROOT_POSE, &[frame], 1.5, 0)
}

fn parsed_anim(bytes: &[u8]) -> Animation {
    let anim = Animation::parse(bytes);
    assert!(
        anim.is_ok(),
        "Animation::parse must succeed, got {:?}",
        anim.as_ref().err()
    );
    anim.expect("parsed: asserted above")
}

/// Pins `Error::BadMagic` for animations: a full-length file that does not
/// start with `ANIM` is rejected with the four bytes it does start with,
/// including a mesh handed to the animation parser.
#[test]
fn sca_animation_wrong_magic_reports_the_found_bytes() {
    let mut buf = minimal_anim();
    buf[..4].copy_from_slice(b"XXXX");
    assert!(
        matches!(Animation::parse(&buf), Err(Error::BadMagic { found }) if found == *b"XXXX"),
        "got {:?}",
        Animation::parse(&buf).err()
    );
    let mesh = minimal_mesh();
    assert!(
        matches!(Animation::parse(&mesh), Err(Error::BadMagic { found }) if found == *b"MODL"),
        "got {:?}",
        Animation::parse(&mesh).err()
    );
}

/// Pins `Error::Truncated` for an animation header: a buffer shorter than
/// the 36-byte header reports needing exactly 36 bytes and the length it has.
#[test]
fn sca_animation_header_shorter_than_36_bytes_is_truncated() {
    let full = minimal_anim();
    for len in [0, 4, 35] {
        let result = Animation::parse(&full[..len]);
        assert!(
            matches!(result, Err(Error::Truncated { needed: 36, len: l }) if l == len),
            "{len} bytes: got {:?}",
            result.err()
        );
    }
}

/// Pins `Error::UnsupportedVersion` for animations: versions 3, 4, and 6 are
/// rejected with the version the header carries; only 5 is read.
#[test]
fn sca_animation_version_other_than_5_is_unsupported() {
    for found in [3, 4, 6] {
        let mut buf = minimal_anim();
        set_u32(&mut buf, VERSION_AT, found);
        let result = Animation::parse(&buf);
        assert!(
            matches!(result, Err(Error::UnsupportedVersion { found: v }) if v == found),
            "version {found}: got {:?}",
            result.err()
        );
    }
}

/// Pins every field of the minimal animation (one bone, one frame): the
/// header's frame count and duration, the bone name and link, the root pose
/// (which is not a frame), and the frame's key time and pose.
#[test]
fn sca_minimal_animation_parses_and_round_trips_its_fields() {
    let anim = parsed_anim(&minimal_anim());
    let expected = Animation {
        frame_count: 1,
        duration: 1.5,
        bone_names: vec!["root".to_owned()],
        bone_links: vec![-1],
        root: pose(&ROOT_POSE),
        frames: vec![Frame {
            time: 0.5,
            poses: vec![pose(&[1.0, 2.0, 3.0, 0.0, 0.0, 0.0, 1.0])],
        }],
    };
    assert_eq!(anim, expected);
}

/// Pins ordering: with two bones and two frames, names and links keep file
/// order, each frame carries one pose per bone in that order, frames keep
/// file order, and the root pose is not counted among the frames.
#[test]
fn sca_animation_two_bones_and_two_frames_keep_their_order() {
    let frames = [
        (
            0.0,
            vec![
                [1.0, 2.0, 3.0, 0.0, 0.0, 0.0, 1.0],
                [4.0, 5.0, 6.0, 0.5, 0.5, 0.5, 0.5],
            ],
        ),
        (
            0.25,
            vec![
                [7.0, 8.0, 9.0, 0.0, 0.0, 1.0, 0.0],
                [10.0, 11.0, 12.0, 1.0, 0.0, 0.0, 0.0],
            ],
        ),
    ];
    let buf = anim_bytes(&["root", "arm"], &[-1, 0], &ROOT_POSE, &frames, 0.25, 0);
    let anim = parsed_anim(&buf);
    assert_eq!(anim.frame_count, 2);
    assert_eq!(anim.bone_names, vec!["root".to_owned(), "arm".to_owned()]);
    assert_eq!(anim.bone_links, vec![-1, 0]);
    assert_eq!(anim.root, pose(&ROOT_POSE));
    assert_eq!(anim.frames.len(), 2);
    for (parsed, (time, poses)) in anim.frames.iter().zip(&frames) {
        assert_eq!(parsed.time.to_bits(), time.to_bits());
        let expected: Vec<Pose> = poses.iter().map(pose).collect();
        assert_eq!(parsed.poses, expected);
    }
}

/// Pins that the header's offsets place the sections: with filler bytes
/// between names, links, and data, the parse equals the compact one.
#[test]
fn sca_animation_honours_section_offsets_over_adjacency() {
    let frame = (0.5, vec![[1.0, 2.0, 3.0, 0.0, 0.0, 0.0, 1.0]]);
    let spaced = anim_bytes(&["root"], &[-1], &ROOT_POSE, &[frame], 1.5, 5);
    assert_eq!(parsed_anim(&spaced), parsed_anim(&minimal_anim()));
}

/// Pins `Error::Malformed` for a frame size that contradicts the bone count:
/// `frame_size` must equal `8 + 28 * bone_count`, as it does in all 475
/// installed animations.
#[test]
fn sca_animation_frame_size_contradicting_bone_count_is_malformed() {
    let mut buf = minimal_anim();
    set_u32(&mut buf, FRAME_SIZE_AT, 8 + 28 * 2);
    let result = Animation::parse(&buf);
    assert!(
        matches!(result, Err(Error::Malformed { .. })),
        "got {:?}",
        result.err()
    );
}

/// Pins `Error::Truncated` for the frame data: a header claiming a million
/// frames over a buffer that holds one reports exactly
/// `animation_data_offset + 28 + frame_count * frame_size` as `needed`, and
/// does not try to allocate that many frames.
#[test]
fn sca_animation_frames_past_the_buffer_are_truncated() {
    let mut buf = minimal_anim();
    set_u32(&mut buf, FRAME_COUNT_AT, 1_000_000);
    let data_offset = ANIM_HEADER_LEN + "root\0".len() + 4;
    let needed = data_offset + POSE_LEN + 1_000_000 * (8 + POSE_LEN);
    let len = buf.len();
    assert!(
        matches!(Animation::parse(&buf), Err(Error::Truncated { needed: n, len: l }) if n == needed && l == len),
        "expected Truncated {{ needed: {needed}, len: {len} }}, got {:?}",
        Animation::parse(&buf).err()
    );
}

/// Pins totality: every proper prefix of a valid animation is `Truncated`,
/// never a panic and never a success, and reports the length it was given.
#[test]
fn sca_animation_every_proper_prefix_is_truncated() {
    let full = minimal_anim();
    for len in 0..full.len() {
        let result = Animation::parse(&full[..len]);
        assert!(
            matches!(result, Err(Error::Truncated { needed, len: l }) if l == len && needed > len),
            "prefix of {len} bytes: got {:?}",
            result.err()
        );
    }
}
