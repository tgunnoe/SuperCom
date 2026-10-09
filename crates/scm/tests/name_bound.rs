//! Both sides of the bone-name bound, for meshes and animations: a name of
//! exactly 1024 bytes parses and keeps its length, one byte more is
//! [`Error::Malformed`]. The bound keeps the memory a crafted file can ask
//! for linear in its size; these tests keep the bound from drifting.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use supercom_scm::{Animation, Error, Mesh};

const MAX_NAME_LEN: usize = 1024;
const MESH_HEADER_LEN: usize = 48;
const ANIM_HEADER_LEN: usize = 36;
const BONE_LEN: usize = 108;
const POSE_LEN: usize = 28;

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
    u32::try_from(len).expect("a synthetic buffer is small")
}

/// A name of `len` bytes, all `b'n'`.
fn name(len: usize) -> Vec<u8> {
    vec![b'n'; len]
}

/// A version-5 mesh with one root bone named `name`, no vertices and no
/// indices: header, the NUL-terminated name, then the bone record.
fn mesh_with_name(name: &[u8]) -> Vec<u8> {
    let bone_offset = MESH_HEADER_LEN + name.len() + 1;
    let table_end = bone_offset + BONE_LEN;
    let mut out = b"MODL".to_vec();
    for field in [
        5,
        len32(bone_offset),
        1,
        len32(table_end),
        len32(table_end),
        0,
        len32(table_end),
        0,
        len32(table_end),
        0,
        1,
    ] {
        put_u32(&mut out, field);
    }
    out.extend_from_slice(name);
    out.push(0);
    for row in [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ] {
        put_f32s(&mut out, &row);
    }
    put_f32s(&mut out, &[0.0, 0.0, 0.0]);
    put_f32s(&mut out, &[0.0, 0.0, 0.0, 1.0]);
    put_i32(
        &mut out,
        i32::try_from(MESH_HEADER_LEN).expect("a small offset"),
    );
    put_i32(&mut out, -1);
    put_i32(&mut out, 0);
    put_i32(&mut out, 0);
    out
}

/// A version-5 animation with one bone named `name`, one link, the root
/// pose and one frame.
fn anim_with_name(name: &[u8]) -> Vec<u8> {
    let names_offset = ANIM_HEADER_LEN;
    let links_offset = names_offset + name.len() + 1;
    let data_offset = links_offset + 4;
    let mut out = b"ANIM".to_vec();
    put_i32(&mut out, 5);
    put_u32(&mut out, 1);
    put_f32s(&mut out, &[1.0]);
    put_u32(&mut out, 1);
    put_u32(&mut out, len32(names_offset));
    put_u32(&mut out, len32(links_offset));
    put_u32(&mut out, len32(data_offset));
    put_u32(&mut out, len32(8 + POSE_LEN));
    out.extend_from_slice(name);
    out.push(0);
    put_i32(&mut out, -1);
    put_f32s(&mut out, &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]);
    put_f32s(&mut out, &[0.0]);
    put_i32(&mut out, 0);
    put_f32s(&mut out, &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]);
    out
}

/// A mesh bone name of exactly the bound parses and keeps its length.
#[test]
fn scm_mesh_bone_name_of_1024_bytes_parses() {
    let mesh = Mesh::parse(&mesh_with_name(&name(MAX_NAME_LEN)));
    let mesh = mesh.expect("a name at the bound parses");
    assert_eq!(mesh.bones.len(), 1);
    assert_eq!(mesh.bones[0].name.len(), MAX_NAME_LEN);
}

/// A mesh bone name one byte over the bound is malformed.
#[test]
fn scm_mesh_bone_name_of_1025_bytes_is_malformed() {
    let result = Mesh::parse(&mesh_with_name(&name(MAX_NAME_LEN + 1)));
    assert!(
        matches!(result, Err(Error::Malformed { .. })),
        "got {result:?}"
    );
}

/// An animation bone name of exactly the bound parses and keeps its length.
#[test]
fn scm_anim_bone_name_of_1024_bytes_parses() {
    let anim = Animation::parse(&anim_with_name(&name(MAX_NAME_LEN)));
    let anim = anim.expect("a name at the bound parses");
    assert_eq!(anim.bone_names.len(), 1);
    assert_eq!(anim.bone_names[0].len(), MAX_NAME_LEN);
}

/// An animation bone name one byte over the bound is malformed.
#[test]
fn scm_anim_bone_name_of_1025_bytes_is_malformed() {
    let result = Animation::parse(&anim_with_name(&name(MAX_NAME_LEN + 1)));
    assert!(
        matches!(result, Err(Error::Malformed { .. })),
        "got {result:?}"
    );
}
