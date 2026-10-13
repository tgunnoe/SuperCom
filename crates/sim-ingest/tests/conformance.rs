//! Conformance of the world loader against the installed game. Each test is
//! game-dependent: it skips, loudly, without the install and fails if
//! `SUPERCOM_REQUIRE_GAME=1`. Counts and identifiers only; no game content
//! is kept in the tree.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha2::{Digest as _, Sha256};
use supercom_sim::{
    Army, BuildRestriction, Faction, Layer, MarkerKind, MotionType, Q, UnitKind, WeaponKind, World,
};
use supercom_sim_ingest::load_world;

/// The raw-sample slope that equals `MaxSlope` 0.75 at the scale 2^-7.
const MAX_SLOPE_RAW: i32 = 96;

fn load(fa: &Path, cache: &Path, map: &str) -> Arc<World> {
    load_world(fa, cache, map).unwrap_or_else(|e| panic!("load_world({map}): {e}"))
}

/// The file in `folder` whose name ends `suffix`, matched case-insensitively.
fn map_file(fa: &Path, folder: &str, suffix: &str) -> PathBuf {
    let dir = fa.join("maps").join(folder);
    let mut found: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot list {}: {e}", dir.display()))
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.to_lowercase().ends_with(suffix))
        })
        .collect();
    assert_eq!(found.len(), 1, "files ending {suffix} in {folder}");
    found.remove(0)
}

// ---- an independent encoder of the documented catalog layout --------------

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    put_u32(out, u32::try_from(s.len()).expect("a short string"));
    out.extend_from_slice(s.as_bytes());
}

fn put_q(out: &mut Vec<u8>, q: Q) {
    out.extend_from_slice(&q.to_bits().to_le_bytes());
}

fn put_f64(out: &mut Vec<u8>, x: f64) {
    let bits = if x.is_nan() {
        0x7ff8_0000_0000_0000
    } else {
        x.to_bits()
    };
    out.extend_from_slice(&bits.to_le_bytes());
}

fn put_opt<T>(out: &mut Vec<u8>, v: Option<T>, put: impl FnOnce(&mut Vec<u8>, T)) {
    match v {
        None => out.push(0),
        Some(v) => {
            out.push(1);
            put(out, v);
        }
    }
}

fn put_set<'a>(out: &mut Vec<u8>, items: impl ExactSizeIterator<Item = &'a String>) {
    put_u32(out, u32::try_from(items.len()).expect("a short set"));
    for s in items {
        put_str(out, s);
    }
}

fn put_weapon(out: &mut Vec<u8>, w: &WeaponKind) {
    put_opt(out, w.label.as_deref(), put_str);
    put_q(out, w.damage);
    put_opt(out, w.damage_radius, put_f64);
    put_str(out, &w.damage_type);
    for v in [
        w.rate_of_fire,
        w.max_radius,
        w.min_radius,
        w.muzzle_velocity,
    ] {
        put_opt(out, v, put_f64);
    }
    put_opt(out, w.projectile_id.as_deref(), put_str);
    out.push(w.target_layers.bits());
    out.push(u8::from(w.manual_fire));
}

fn put_unit(out: &mut Vec<u8>, u: &UnitKind) {
    put_str(out, &u.id);
    put_set(out, u.categories.iter());
    put_q(out, u.build_cost_mass);
    put_q(out, u.build_cost_energy);
    for v in [
        u.build_time,
        u.build_rate,
        u.production_mass,
        u.production_energy,
        u.storage_mass,
        u.storage_energy,
        u.maintenance_energy,
    ] {
        put_opt(out, v, put_q);
    }
    put_set(out, u.buildable.iter());
    out.push(u.motion as u8);
    for v in [
        u.max_speed,
        u.turn_rate,
        u.max_acceleration,
        u.max_brake,
        u.elevation,
    ] {
        put_opt(out, v, put_f64);
    }
    out.push(u.build_restriction as u8);
    out.push(u.build_on.bits());
    put_opt(out, u.footprint, |o, (x, z)| {
        put_u32(o, x);
        put_u32(o, z);
    });
    put_q(out, u.health);
    put_q(out, u.max_health);
    put_opt(out, u.regen_rate, put_q);
    put_str(out, &u.armor_type);
    put_opt(out, u.vision_radius, put_f64);
    put_u32(out, u.command_caps.bits());
    put_u32(out, u32::try_from(u.weapons.len()).expect("few weapons"));
    for w in &u.weapons {
        put_weapon(out, w);
    }
}

fn expected_catalog_digest(world: &World) -> [u8; 32] {
    let mut out = Vec::new();
    put_str(&mut out, "catalog-v1");
    put_u32(&mut out, u32::try_from(world.units.len()).expect("units"));
    for unit in world.units.values() {
        put_unit(&mut out, unit);
    }
    put_u32(
        &mut out,
        u32::try_from(world.projectiles.len()).expect("projectiles"),
    );
    for p in world.projectiles.values() {
        put_str(&mut out, &p.id);
        put_set(&mut out, p.categories.iter());
        for v in [
            p.initial_speed,
            p.max_speed,
            p.acceleration,
            p.turn_rate,
            p.lifetime,
        ] {
            put_opt(&mut out, v, put_f64);
        }
        out.extend([p.track_target, p.use_gravity, p.destroy_on_water].map(u8::from));
    }
    Sha256::digest(&out).into()
}

// ---- the tests ----------------------------------------------------------------

#[test]
fn conformance_ingest_world_from_the_installed_catalog() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let cache = tempfile::tempdir().expect("a cache directory");
    let world = load(&fa, cache.path(), "SCMP_012");
    assert_eq!(world.units.len(), 568, "unit kinds");
    assert_eq!(world.projectiles.len(), 365, "projectile kinds");

    // SIM-029's census of motion types, and SIM-049's build restrictions.
    let motion = |m: MotionType| world.units.values().filter(|u| u.motion == m).count();
    for (m, n) in [
        (MotionType::None, 374),
        (MotionType::Air, 60),
        (MotionType::Land, 50),
        (MotionType::Water, 27),
        (MotionType::Hover, 19),
        (MotionType::Amphibious, 17),
        (MotionType::SurfacingSub, 13),
        (MotionType::AmphibiousFloating, 8),
    ] {
        assert_eq!(motion(m), n, "{m:?}");
    }
    let restricted = |r: BuildRestriction| {
        world
            .units
            .values()
            .filter(|u| u.build_restriction == r)
            .count()
    };
    assert_eq!(restricted(BuildRestriction::OnMassDeposit), 12);
    assert_eq!(restricted(BuildRestriction::OnHydrocarbonDeposit), 4);
    assert_eq!(restricted(BuildRestriction::None), 568 - 16);

    // The catalog digest is stable across two loads (the second reads the
    // cache), names a catalog, and equals an independent encoding of the
    // documented layout.
    let again = load(&fa, cache.path(), "SCMP_012");
    assert_eq!(world.catalog_digest, again.catalog_digest);
    assert_ne!(world.catalog_digest, [0; 32]);
    assert_eq!(world.catalog_digest, expected_catalog_digest(&world));
    assert_eq!(
        world.catalog_digest,
        supercom_sim::catalog_digest(&world.units, &world.projectiles)
    );
}

#[test]
fn conformance_ingest_map_and_markers() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let cache = tempfile::tempdir().expect("a cache directory");

    let world = load(&fa, cache.path(), "SCMP_012");
    assert_eq!((world.map.width, world.map.height), (256, 256));
    assert_eq!(world.map_name, "SCMP_012");
    assert_ne!(world.map_digest, [0; 32]);
    let count = |kind: MarkerKind| world.markers.iter().filter(|m| m.kind == kind).count();
    assert_eq!(count(MarkerKind::Mass), 24);
    assert_eq!(count(MarkerKind::Hydrocarbon), 2);
    let army = |n: u8| {
        world
            .markers
            .iter()
            .find(|m| m.kind == MarkerKind::Army(n))
            .unwrap_or_else(|| panic!("marker ARMY_{n}"))
    };
    assert_eq!(army(1).name, "ARMY_1");
    assert_eq!(
        (army(1).pos.x, army(1).pos.y, army(1).pos.z),
        (14.5, f64::from(6.748_05_f32), 241.5)
    );
    assert_eq!(
        (army(2).pos.x, army(2).pos.y, army(2).pos.z),
        (241.5, 6.75, 14.5)
    );

    // The grid against the file's own samples and the test's own gradient.
    let bytes = fs::read(map_file(&fa, "SCMP_012", ".scmap")).expect("the map file");
    let map = supercom_scmap::Map::parse(&bytes).expect("the map parses");
    let samples = &map.heightmap.samples;
    assert_eq!(world.map.heights.len(), samples.len());
    for (i, (q, s)) in world.map.heights.iter().zip(samples).enumerate() {
        assert_eq!(q.to_bits(), i64::from(*s) << 25, "height sample {i}");
    }
    let stride = 257_usize;
    let slope_ok = |x: usize, z: usize| {
        let at = |cx: usize, cz: usize| i32::from(samples[cz * stride + cx]);
        let c = [at(x, z), at(x + 1, z), at(x + 1, z + 1), at(x, z + 1)];
        (0..4).all(|i| (c[i] - c[(i + 1) % 4]).abs() <= MAX_SLOPE_RAW)
    };
    let mut expected = 0_usize;
    for z in 0..256_usize {
        for x in 0..256_usize {
            let ok = slope_ok(x, z);
            expected += usize::from(ok);
            let (cx, cz) = (u32::try_from(x).expect("x"), u32::try_from(z).expect("z"));
            assert_eq!(
                world.map.passable(Layer::Land, cx, cz),
                ok,
                "cell ({x}, {z})"
            );
        }
    }
    let land = world.map.passable.get(&Layer::Land).expect("a land grid");
    assert_eq!(land.iter().filter(|c| **c).count(), expected);
    assert!(world.map.passable.contains_key(&Layer::Air));
    for n in [1, 2] {
        let p = army(n).pos;
        // Truncation is the intent: the cell holding a non-negative position.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (cx, cz) = (p.x as u32, p.z as u32);
        assert!(world.map.passable(Layer::Land, cx, cz), "ARMY_{n} cell");
    }

    // The larger maps: size, resource markers, and army counts.
    for (folder, size, mass, hydro, armies) in
        [("SCMP_007", 512, 68, 6, 6), ("SCMP_003", 1024, 68, 4, 4)]
    {
        let w = load(&fa, cache.path(), folder);
        assert_eq!((w.map.width, w.map.height), (size, size), "{folder}");
        let n = |k: fn(&MarkerKind) -> bool| w.markers.iter().filter(|m| k(&m.kind)).count();
        assert_eq!(n(|k| *k == MarkerKind::Mass), mass, "{folder} mass");
        assert_eq!(
            n(|k| *k == MarkerKind::Hydrocarbon),
            hydro,
            "{folder} hydro"
        );
        assert_eq!(
            n(|k| matches!(k, MarkerKind::Army(_))),
            armies,
            "{folder} armies"
        );
    }
}

#[test]
fn conformance_ingest_skirmish_on_scmp_012() {
    let Some(fa) = supercom_testkit::require_game() else {
        return;
    };
    let cache = tempfile::tempdir().expect("a cache directory");
    let world = load(&fa, cache.path(), "SCMP_012");
    let state = Arc::clone(&world)
        .skirmish(1, &[Faction::UEF, Faction::Cybran])
        .expect("a UEF and Cybran skirmish");
    assert_eq!(state.armies.len(), 2);
    assert_eq!(state.entities.len(), 2);
    for (slot, id, health) in [(1_u8, "uel0001", 12_000), (2, "url0001", 10_000)] {
        let kind = world.units.get(id).expect("the commander kind");
        assert_eq!(kind.storage_mass, Some(Q::from_num(650)), "{id} storage");
        assert_eq!(kind.storage_energy, Some(Q::from_num(4000)), "{id} storage");
        assert_eq!(kind.max_health, Q::from_num(health), "{id} health");
        let army = &state.armies[&Army(slot)];
        assert_eq!(army.mass, Q::from_num(650));
        assert_eq!(army.energy, Q::from_num(4000));
        assert_eq!(army.mass_storage, Q::from_num(650));
        assert_eq!(army.energy_storage, Q::from_num(4000));
        let unit = state
            .entities
            .values()
            .find(|e| e.army == Army(slot))
            .expect("the army's commander");
        assert_eq!(unit.blueprint, id);
        assert_eq!(unit.health, Q::from_num(health));
        let marker = world
            .markers
            .iter()
            .find(|m| m.kind == MarkerKind::Army(slot))
            .expect("the army's marker");
        assert_eq!(unit.pos, marker.pos, "{id} at ARMY_{slot}");
    }
}
