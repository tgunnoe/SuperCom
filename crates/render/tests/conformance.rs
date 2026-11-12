//! The renderer's mesh and water plane over the installed maps. Skips when
//! the game is not installed; with `SUPERCOM_REQUIRE_GAME=1` it fails
//! instead.

use std::fs;
use std::path::Path;
use supercom_render::{TerrainMesh, WaterPlane};
use supercom_scmap::Map;

/// Reads the one `.scmap` file of an installed map folder.
#[expect(
    clippy::expect_used,
    reason = "test helper; clippy.toml exempts only #[test] functions"
)]
fn load(fa: &Path, folder: &str) -> Map {
    let dir = fa.join("maps").join(folder);
    let file = fs::read_dir(&dir)
        .expect("the map folder lists")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|ext| ext == "scmap"))
        .expect("the folder holds a .scmap file");
    let bytes = fs::read(&file).expect("the map file reads");
    Map::parse(&bytes).expect("the installed map parses")
}

/// `SCMP_012` and `SCMP_016`: 256 by 256 cells, so 66,049 vertices and
/// 393,216 indices; the first spans samples 442 to 2581 and carries no
/// water, the second has its surface at elevation 17.5.
#[test]
fn conformance_render_installed_maps() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };

    let dry = load(&fa, "SCMP_012");
    let mesh = TerrainMesh::build(&dry);
    assert!(mesh.is_ok(), "SCMP_012 builds: {:?}", mesh.as_ref().err());
    let Ok(mesh) = mesh else { return };
    assert_eq!(mesh.positions.len(), 66_049);
    assert_eq!(mesh.indices.len(), 393_216);
    assert!(
        (mesh.lowest - 3.453_125).abs() < 1e-6,
        "lowest {}",
        mesh.lowest
    );
    assert!(
        (mesh.highest - 20.164_062_5).abs() < 1e-6,
        "highest {}",
        mesh.highest
    );
    assert!(WaterPlane::from_map(&dry).is_none());

    let wet = load(&fa, "SCMP_016");
    let plane = WaterPlane::from_map(&wet);
    assert!(plane.is_some(), "SCMP_016 carries water");
    let corners = plane.map_or([[f32::NAN; 3]; 4], |p| p.corners);
    assert!(
        corners.iter().all(|c| (c[1] - 17.5).abs() < 1e-6),
        "{corners:?}"
    );
}
