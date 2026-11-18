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

use std::collections::BTreeMap;
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

/// What the three vectors after a vertex's position say about the triangles
/// of one mesh, in the order the `Vertex` fields `normal`, `tangent`,
/// `binormal` name them.
struct VectorCensus {
    /// Triangles whose face normal is not exactly zero.
    live: u32,
    /// Live triangles whose face normal has a positive dot product with the
    /// sum of the three corners' vector.
    agree: [u32; 3],
    /// Sum of the absolute cosines over the live triangles whose sum is
    /// not the zero vector.
    cosine_sum: [f64; 3],
    /// How many terms went into each `cosine_sum`.
    cosine_terms: [u32; 3],
    /// Vertices whose first vector is shorter than 1e-6.
    short_first: u32,
}

impl VectorCensus {
    fn share(&self, which: usize) -> f64 {
        f64::from(self.agree[which]) / f64::from(self.live)
    }

    fn mean_cosine(&self, which: usize) -> f64 {
        if self.cosine_terms[which] == 0 {
            0.0
        } else {
            self.cosine_sum[which] / f64::from(self.cosine_terms[which])
        }
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Counts, for each of the three vectors, the triangles it agrees with:
/// the face normal of corners a, b, c is (b - a) x (c - a) in f64 from the
/// stored f32 values, zero exactly for a degenerate triangle.
fn vector_census(mesh: &Mesh) -> VectorCensus {
    let mut census = VectorCensus {
        live: 0,
        agree: [0; 3],
        cosine_sum: [0.0; 3],
        cosine_terms: [0; 3],
        short_first: 0,
    };
    let widen = |v: [f32; 3]| v.map(f64::from);
    for corner in mesh.indices.chunks_exact(3) {
        let vertices: Vec<_> = corner
            .iter()
            .filter_map(|&i| mesh.vertices.get(usize::from(i)))
            .collect();
        if vertices.len() != 3 {
            continue;
        }
        let [origin, second, third] = [0, 1, 2].map(|k| widen(vertices[k].position));
        let to_second = [0, 1, 2].map(|k| second[k] - origin[k]);
        let to_third = [0, 1, 2].map(|k| third[k] - origin[k]);
        let face = [
            to_second[1] * to_third[2] - to_second[2] * to_third[1],
            to_second[2] * to_third[0] - to_second[0] * to_third[2],
            to_second[0] * to_third[1] - to_second[1] * to_third[0],
        ];
        let face_length = dot(face, face).sqrt();
        if face_length == 0.0 {
            continue;
        }
        census.live += 1;
        for which in 0..3 {
            let mut sum = [0.0; 3];
            for vertex in &vertices {
                let vector = widen([vertex.normal, vertex.tangent, vertex.binormal][which]);
                for k in 0..3 {
                    sum[k] += vector[k];
                }
            }
            let along = dot(face, sum);
            let sum_length = dot(sum, sum).sqrt();
            if along > 0.0 {
                census.agree[which] += 1;
            }
            if sum_length > 0.0 {
                census.cosine_sum[which] += (along / (face_length * sum_length)).abs();
                census.cosine_terms[which] += 1;
            }
        }
    }
    census.short_first = u32::try_from(
        mesh.vertices
            .iter()
            .filter(|v| dot(v.normal.map(f64::from), v.normal.map(f64::from)).sqrt() < 1e-6)
            .count(),
    )
    .unwrap_or(u32::MAX);
    census
}

/// Pins the order of a vertex's vectors (FMT-060): over the install's 1,938
/// version-5 meshes the second stored vector, which `Vertex::normal` holds,
/// has the greatest mean absolute cosine with the face normal; 1,839 meshes
/// agree with it on a share of at least 0.99 and 99 fall below (74 under
/// `env`, 5 under `projectiles`, 20 under `units`), the lowest at 0.8800;
/// nine vertices have it shorter than 1e-6, in three named meshes.
#[test]
fn conformance_scm_vertex_vectors_order() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let vfs = mounted(&fa);

    let (mut cosine_first, mut meshes, mut at_least, mut below) = (0, 0, 0, 0);
    let mut by_directory: BTreeMap<String, u32> = BTreeMap::new();
    let mut lowest = (f64::INFINITY, String::new());
    let mut short: Vec<(String, u32)> = Vec::new();
    each_v5_mesh(&vfs, |name, _, mesh| {
        let census = vector_census(mesh);
        if census.live == 0 {
            return;
        }
        meshes += 1;
        if census.mean_cosine(0) > census.mean_cosine(1)
            && census.mean_cosine(0) > census.mean_cosine(2)
        {
            cosine_first += 1;
        }
        let share = census.share(0);
        if share >= 0.99 {
            at_least += 1;
        } else {
            below += 1;
            let top = name.split('/').next().unwrap_or("").to_lowercase();
            *by_directory.entry(top).or_insert(0) += 1;
        }
        if share < lowest.0 {
            lowest = (share, name.to_owned());
        }
        if census.short_first > 0 {
            short.push((name.to_owned(), census.short_first));
        }
    });

    assert_eq!(
        cosine_first, MESHES_V5,
        "meshes whose second stored vector has the greatest mean cosine"
    );
    assert_eq!(meshes, MESHES_V5, "meshes with a non-degenerate triangle");
    assert_eq!(at_least, 1839, "meshes agreeing on a share of 0.99 or more");
    assert_eq!(below, 99, "meshes below a share of 0.99");
    assert_eq!(
        by_directory,
        BTreeMap::from([
            ("env".to_owned(), 74),
            ("projectiles".to_owned(), 5),
            ("units".to_owned(), 20),
        ]),
        "the meshes below 0.99, by top directory"
    );
    assert!(
        (lowest.0 - 0.8800).abs() < 1e-4,
        "lowest share {} is not 0.8800",
        lowest.0
    );
    assert_eq!(lowest.1, "env/Evergreen/Props/Trees/Pine06_s2_lod1.scm");
    assert_eq!(
        short,
        vec![
            ("meshes/game/map-border_squ_cybran_a_lod0.scm".to_owned(), 3),
            ("units/UXB0008/UXB0008_LOD0.scm".to_owned(), 2),
            ("units/UXB0009/UXB0009_LOD0.scm".to_owned(), 4),
        ],
        "vertices whose second stored vector is shorter than 1e-6"
    );
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
