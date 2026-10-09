//! Conformance of `supercom-scm` against the installed game.
//!
//! Every test here is game-dependent: it begins by asking `supercom_testkit`
//! for the install and returns (skips, loudly) when there is none. Run them
//! against the real game with `SUPERCOM_REQUIRE_GAME=1`.
//!
//! The expected numbers are measured on the installed Forged Alliance:
//! 1,939 `.scm` members across the archives, 1,938 of them `MODL` version 5
//! and one version 3; 475 `.sca` members, all `ANIM` version 5. No game
//! content is stored here: names, counts, and sizes only.
//!
//! Each test's first assertion is on the behaviour under test, never on the
//! environment, so a failure points at the capability and not at the install.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::path::Path;

use supercom_scd::Vfs;
use supercom_scm::{Animation, Error, Mesh};

/// Version-5 meshes in the install.
const MESHES_V5: usize = 1938;
/// Bones across all version-5 meshes (`total_bone_count` summed).
const MESH_BONES: usize = 10_752;
/// Vertices across all version-5 meshes.
const MESH_VERTICES: usize = 1_812_633;
/// Animations in the install.
const ANIMATIONS: usize = 475;
/// Key frames across all animations.
const ANIM_FRAMES: usize = 112_623;
/// Poses across all frames of all animations.
const ANIM_POSES: usize = 1_777_774;

/// Mounts the install, failing the calling test with the error if it cannot.
fn mounted(fa: &Path) -> Vfs {
    let vfs = Vfs::from_install(fa);
    assert!(
        vfs.is_ok(),
        "Vfs::from_install must mount the install, got {:?}",
        vfs.as_ref().err()
    );
    vfs.expect("mounted: asserted above")
}

/// The visible files whose names end in `extension` (lowercase, with the
/// dot), in name order.
fn members(vfs: &Vfs, extension: &str) -> Vec<String> {
    let mut names: Vec<String> = vfs
        .files()
        .filter(|entry| entry.name.to_lowercase().ends_with(extension))
        .map(|entry| entry.name.clone())
        .collect();
    names.sort();
    names
}

/// Reads one member, failing the calling test with the error if it cannot.
fn bytes_of(vfs: &Vfs, name: &str) -> Vec<u8> {
    let read = vfs.read(name);
    assert!(read.is_ok(), "{name} unreadable: {:?}", read.as_ref().err());
    read.expect("read: asserted above")
}

/// The little-endian `u32` at `at`, read without the code under test.
fn le_u32(bytes: &[u8], at: usize) -> u32 {
    let word: [u8; 4] = bytes[at..at + 4].try_into().expect("four bytes");
    u32::from_le_bytes(word)
}

fn le_f32(bytes: &[u8], at: usize) -> f32 {
    f32::from_bits(le_u32(bytes, at))
}

fn as_usize(value: u32) -> usize {
    usize::try_from(value).expect("a u32 fits in usize")
}

/// Whether the quaternion has unit length to within 0.01.
fn is_unit(q: &[f32; 4]) -> bool {
    let norm = q
        .iter()
        .map(|&x| f64::from(x) * f64::from(x))
        .sum::<f64>()
        .sqrt();
    (norm - 1.0).abs() < 0.01
}

/// Parses every version-5 mesh, failing the calling test at the first parse
/// error, and hands each (name, raw bytes, mesh) to `visit`. Returns how many
/// meshes were visited.
fn each_v5_mesh(vfs: &Vfs, mut visit: impl FnMut(&str, &[u8], &Mesh)) -> usize {
    let mut visited = 0;
    for name in members(vfs, ".scm") {
        let bytes = bytes_of(vfs, &name);
        if le_u32(&bytes, 4) != 5 {
            continue;
        }
        let mesh = Mesh::parse(&bytes);
        assert!(
            mesh.is_ok(),
            "{name} must parse, got {:?}",
            mesh.as_ref().err()
        );
        visit(&name, &bytes, &mesh.expect("parsed: asserted above"));
        visited += 1;
    }
    visited
}

/// Parses every animation, failing the calling test at the first parse
/// error, and hands each (name, raw bytes, animation) to `visit`. Returns how
/// many were visited.
fn each_animation(vfs: &Vfs, mut visit: impl FnMut(&str, &[u8], &Animation)) -> usize {
    let mut visited = 0;
    for name in members(vfs, ".sca") {
        let bytes = bytes_of(vfs, &name);
        let anim = Animation::parse(&bytes);
        assert!(
            anim.is_ok(),
            "{name} must parse, got {:?}",
            anim.as_ref().err()
        );
        visit(&name, &bytes, &anim.expect("parsed: asserted above"));
        visited += 1;
    }
    visited
}

/// Pins `Mesh::parse` over the install: all 1,938 version-5 `.scm` members
/// parse, and the one version-3 member (1,905 bytes) is rejected with
/// `UnsupportedVersion { found: 3 }`. The install holds 1,939 `.scm` members.
#[test]
fn conformance_scm_every_mesh_parses() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = mounted(&fa);

    let visited = each_v5_mesh(&vfs, |_, _, _| {});
    assert_eq!(visited, MESHES_V5, "version-5 meshes parsed");

    let odd: Vec<(String, Vec<u8>)> = members(&vfs, ".scm")
        .into_iter()
        .map(|name| {
            let bytes = bytes_of(&vfs, &name);
            (name, bytes)
        })
        .filter(|(_, bytes)| le_u32(bytes, 4) != 5)
        .collect();
    assert_eq!(odd.len(), 1, "exactly one .scm member is not version 5");
    let (name, bytes) = &odd[0];
    assert_eq!(bytes.len(), 1905, "{name}: the version-3 member's size");
    let result = Mesh::parse(bytes);
    assert!(
        matches!(result, Err(Error::UnsupportedVersion { found: 3 })),
        "{name}: expected UnsupportedVersion {{ found: 3 }}, got {:?}",
        result.err()
    );
}

/// Pins the index and bone-index ranges: in every version-5 mesh, every
/// index is below `vertices.len()`, every vertex's bone indices are below
/// `bones.len()`, and `indices.len()` is a multiple of 3.
#[test]
fn conformance_scm_indices_in_range() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = mounted(&fa);

    let mut violations = Vec::new();
    let visited = each_v5_mesh(&vfs, |name, _, mesh| {
        let vertices = mesh.vertices.len();
        let bones = mesh.bones.len();
        let indices_ok = mesh.indices.iter().all(|&i| usize::from(i) < vertices);
        let skin_ok = mesh
            .vertices
            .iter()
            .flat_map(|v| v.bone_indices)
            .all(|b| usize::from(b) < bones);
        if !(indices_ok && skin_ok && mesh.indices.len() % 3 == 0) {
            violations.push(name.to_owned());
        }
    });
    assert!(
        violations.is_empty(),
        "{} meshes out of range, first: {:?}",
        violations.len(),
        violations.iter().take(3).collect::<Vec<_>>()
    );
    assert_eq!(visited, MESHES_V5, "version-5 meshes checked");
}

/// Pins the mesh tables against the raw header: `bones.len()` is
/// `total_bone_count`, `vertices.len()` is `vertex_count`, `indices.len()` is
/// `index_count`, every bone has a non-empty name and a parent below the bone
/// count, and the totals are 10,752 bones and 1,812,633 vertices.
#[test]
fn conformance_scm_tables_match_the_header() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = mounted(&fa);

    let (mut bones, mut vertices) = (0, 0);
    let mut violations = Vec::new();
    each_v5_mesh(&vfs, |name, raw, mesh| {
        let header_ok = mesh.bones.len() == as_usize(le_u32(raw, 44))
            && mesh.vertices.len() == as_usize(le_u32(raw, 24))
            && mesh.indices.len() == as_usize(le_u32(raw, 32));
        let bones_ok = mesh
            .bones
            .iter()
            .all(|b| !b.name.is_empty() && b.parent.is_none_or(|p| p < mesh.bones.len()));
        if !(header_ok && bones_ok) {
            violations.push(name.to_owned());
        }
        bones += mesh.bones.len();
        vertices += mesh.vertices.len();
    });
    assert!(
        violations.is_empty(),
        "{} meshes disagree with their header, first: {:?}",
        violations.len(),
        violations.iter().take(3).collect::<Vec<_>>()
    );
    assert_eq!(bones, MESH_BONES, "bones across all meshes");
    assert_eq!(vertices, MESH_VERTICES, "vertices across all meshes");
}

/// Pins the bone rotation field: every bone of every version-5 mesh stores a
/// unit quaternion (length 1 within 0.01), which a misplaced or byte-swapped
/// read would not.
#[test]
fn conformance_scm_bone_rotations_are_unit() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = mounted(&fa);

    let mut off = Vec::new();
    each_v5_mesh(&vfs, |name, _, mesh| {
        if !mesh.bones.iter().all(|bone| is_unit(&bone.rotation)) {
            off.push(name.to_owned());
        }
    });
    assert!(
        off.is_empty(),
        "{} meshes hold a non-unit bone rotation, first: {:?}",
        off.len(),
        off.iter().take(3).collect::<Vec<_>>()
    );
}

/// Pins `Animation::parse` over the install: all 475 `.sca` members parse,
/// `frames.len()` equals `frame_count`, and every frame holds one pose per
/// bone name.
#[test]
fn conformance_sca_every_animation_parses() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = mounted(&fa);

    let mut violations = Vec::new();
    let visited = each_animation(&vfs, |name, _, anim| {
        let counts_ok = anim.frames.len() == as_usize(anim.frame_count);
        let poses_ok = anim
            .frames
            .iter()
            .all(|frame| frame.poses.len() == anim.bone_names.len());
        if !(counts_ok && poses_ok) {
            violations.push(name.to_owned());
        }
    });
    assert!(
        violations.is_empty(),
        "{} animations are inconsistent, first: {:?}",
        violations.len(),
        violations.iter().take(3).collect::<Vec<_>>()
    );
    assert_eq!(visited, ANIMATIONS, "animations parsed");
}

/// Pins the animation fields against the raw header and the stored poses:
/// `frame_count`, `duration`, and the bone count match the header; there is
/// one link per bone, each -1 or a bone index; the root pose and every pose
/// hold a unit rotation; key times never decrease and never pass the
/// duration; and the install holds 112,623 frames and 1,777,774 poses.
#[test]
fn conformance_sca_fields_match_the_header_and_poses() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = mounted(&fa);

    let (mut frames, mut poses) = (0, 0);
    let mut violations = Vec::new();
    each_animation(&vfs, |name, raw, anim| {
        let bones = anim.bone_names.len();
        let header_ok = anim.frame_count == le_u32(raw, 8)
            && anim.duration.to_bits() == le_f32(raw, 12).to_bits()
            && bones == as_usize(le_u32(raw, 16));
        let links_ok = anim.bone_links.len() == bones
            && anim
                .bone_links
                .iter()
                .all(|&l| l == -1 || usize::try_from(l).is_ok_and(|l| l < bones));
        let rotations_ok = is_unit(&anim.root.rotation)
            && anim
                .frames
                .iter()
                .flat_map(|frame| &frame.poses)
                .all(|pose| is_unit(&pose.rotation));
        let times_ok = anim.frames.windows(2).all(|w| w[0].time <= w[1].time)
            && anim
                .frames
                .iter()
                .all(|frame| frame.time <= anim.duration + 1e-3);
        if !(header_ok && links_ok && rotations_ok && times_ok) {
            violations.push(name.to_owned());
        }
        frames += anim.frames.len();
        poses += anim.frames.iter().map(|f| f.poses.len()).sum::<usize>();
    });
    assert!(
        violations.is_empty(),
        "{} animations disagree with their header or poses, first: {:?}",
        violations.len(),
        violations.iter().take(3).collect::<Vec<_>>()
    );
    assert_eq!(frames, ANIM_FRAMES, "key frames across all animations");
    assert_eq!(poses, ANIM_POSES, "poses across all animations");
}
