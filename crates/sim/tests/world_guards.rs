//! Guard tests of the sim's static world (docs/specs/sim.md SIM-011,
//! SIM-035, SIM-048 to SIM-051). Each test is named for the defect whose
//! reintroduction it fails. All synthetic: no install, no file read; the
//! installed-data halves live in `crates/sim-ingest/tests/world_guards.rs`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::collections::BTreeSet;
use std::sync::Arc;

use supercom_data as data;
use supercom_sim::{
    Error, Faction, IngestError, Layer, MapGrid, Marker, MarkerKind, Pos, Q, UnitKind, World,
};

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

fn typed_unit(id: &str) -> data::Unit {
    data::Unit {
        id: id.to_owned(),
        categories: strings(&["MOBILE", "LAND"]),
        economy: data::Economy {
            build_cost_energy: 1.0,
            build_cost_mass: 1.0,
            build_time: None,
            build_rate: None,
            production_per_second_energy: None,
            production_per_second_mass: None,
            storage_energy: None,
            storage_mass: None,
            buildable_category: Vec::new(),
            maintenance_consumption_per_second_energy: None,
        },
        physics: data::Physics {
            motion_type: "RULEUMT_Land".to_owned(),
            max_speed: Some(2.0),
            turn_rate: None,
            max_acceleration: None,
            max_brake: None,
            elevation: None,
            min_speed_percent: None,
            build_restriction: None,
            build_on_layer_caps: strings(&["LAYER_Land"]),
        },
        defense: data::Defense {
            health: 100.0,
            max_health: 100.0,
            armor_type: "Normal".to_owned(),
            regen_rate: None,
        },
        intel: None,
        weapons: Vec::new(),
        footprint: None,
        command_caps: Vec::new(),
    }
}

fn typed_weapon(projectile: &str) -> data::Weapon {
    data::Weapon {
        label: None,
        damage: 1.0,
        rate_of_fire: None,
        max_radius: None,
        projectile_id: Some(projectile.to_owned()),
        damage_radius: None,
        damage_type: "Normal".to_owned(),
        damage_friendly: None,
        min_radius: None,
        muzzle_velocity: None,
        projectiles_per_on_fire: None,
        muzzle_salvo_size: None,
        muzzle_salvo_delay: None,
        rack_salvo_reload_time: None,
        rack_salvo_charge_time: None,
        rack_salvo_fires_after_charge: None,
        ballistic_arc: None,
        target_priorities: Vec::new(),
        fire_target_layer_caps: Vec::new(),
        tracking_radius: None,
        firing_tolerance: None,
        target_check_interval: None,
        weapon_category: "Direct Fire".to_owned(),
        turreted: None,
        manual_fire: None,
    }
}

fn typed_projectile(id: &str) -> data::Projectile {
    data::Projectile {
        id: id.to_owned(),
        categories: strings(&["PROJECTILE"]),
        physics: data::ProjectilePhysics {
            initial_speed: None,
            max_speed: None,
            acceleration: None,
            turn_rate: None,
            lifetime: None,
            track_target: None,
            use_gravity: None,
            lead_target: None,
            velocity_align: None,
            destroy_on_water: None,
        },
    }
}

fn build(units: &[data::Unit], projectiles: &[data::Projectile]) -> Result<World, Error> {
    World::from_parts(
        units,
        projectiles,
        World::empty().map,
        Vec::new(),
        "synthetic".to_owned(),
    )
}

fn builder(id: &str, expressions: &[&str]) -> data::Unit {
    let mut unit = typed_unit(id);
    unit.economy.buildable_category = strings(expressions);
    unit
}

fn lower(set: &[&str]) -> BTreeSet<String> {
    set.iter().map(|s| (*s).to_owned()).collect()
}

fn assert_all_lower(keys: impl Iterator<Item = impl AsRef<str>>) {
    for key in keys {
        let key = key.as_ref();
        assert_eq!(key, key.to_ascii_lowercase(), "{key} is not lower case");
    }
}

// ---- SIM-011, SIM-049: one id form ---------------------------------------

#[test]
fn guard_builder_token_matches_an_id_in_either_case() {
    let world = build(
        &[
            builder("BLD0001", &["upp0001", "LOW0002"]),
            typed_unit("UPP0001"),
            typed_unit("low0002"),
            typed_unit("other003"),
        ],
        &[],
    )
    .expect("a valid catalog");
    assert_eq!(
        world.units["bld0001"].buildable,
        lower(&["upp0001", "low0002"]),
        "an upper-case input id is named by a lower-case token and the reverse"
    );
}

#[test]
fn guard_every_buildable_member_is_a_unit_key() {
    let world = build(
        &[
            builder("BLD0001", &["MOBILE LAND", "tgt0001"]),
            typed_unit("TGT0001"),
            typed_unit("Mixed0002"),
        ],
        &[],
    )
    .expect("a valid catalog");
    assert_all_lower(world.units.keys());
    let mut members = 0;
    for kind in world.units.values() {
        for member in &kind.buildable {
            members += 1;
            assert!(
                world.units.contains_key(member),
                "{} lists {member}, which is not a unit key",
                kind.id
            );
        }
    }
    assert!(members > 0, "the catalog selects something");
    assert_eq!(world.units["tgt0001"].id, "tgt0001", "the id field");
}

#[test]
fn guard_empty_expression_selects_nothing() {
    for expression in ["", "   ", " \t "] {
        let world = build(
            &[builder("BLD0001", &[expression]), typed_unit("TGT0001")],
            &[],
        )
        .expect("a valid catalog");
        assert!(
            world.units["bld0001"].buildable.is_empty(),
            "{expression:?} selected {:?}",
            world.units["bld0001"].buildable
        );
    }
}

#[test]
fn guard_weapon_projectile_id_resolves_to_a_projectile_key() {
    let mut shooter = typed_unit("SHT0001");
    shooter.weapons = vec![
        typed_weapon("ProJ1"),
        typed_weapon("proj2"),
        typed_weapon("PROJ3"),
    ];
    let world = build(
        &[shooter],
        &[
            typed_projectile("proj1"),
            typed_projectile("PROJ2"),
            typed_projectile("Proj3"),
        ],
    )
    .expect("a valid catalog");
    assert_all_lower(world.projectiles.keys());
    for weapon in &world.units["sht0001"].weapons {
        let id = weapon.projectile_id.as_deref().expect("a projectile id");
        assert!(
            world.projectiles.contains_key(id),
            "{id} is no projectile key"
        );
    }
}

#[test]
fn guard_duplicate_ids_after_lowering_are_refused() {
    assert_eq!(
        build(&[typed_unit("ABC0001"), typed_unit("abc0001")], &[]),
        Err(Error::Ingest(IngestError::DuplicateUnitId(
            "abc0001".into()
        )))
    );
    assert_eq!(
        build(&[], &[typed_projectile("Proj1"), typed_projectile("PROJ1")]),
        Err(Error::Ingest(IngestError::DuplicateProjectileId(
            "proj1".into()
        )))
    );
    // Distinct ids are not duplicates.
    assert!(
        build(
            &[typed_unit("a"), typed_unit("b")],
            &[typed_projectile("p")]
        )
        .is_ok()
    );
}

// ---- SIM-050: rounding and overflow bounds --------------------------------

#[test]
fn guard_exactly_minus_two_to_the_31_is_refused() {
    let mut unit = typed_unit("MIN0001");
    unit.economy.storage_mass = Some(-2_147_483_648.0);
    match build(&[unit], &[]) {
        Err(Error::Ingest(IngestError::ValueOverflow { path })) => {
            assert!(path.contains("storage_mass"), "{path}");
        }
        other => panic!("expected ValueOverflow, got {other:?}"),
    }
    // The nearest magnitude below the bound converts.
    let mut inside = typed_unit("MIN0002");
    inside.economy.storage_mass = Some(-2_147_483_647.0);
    assert!(build(&[inside], &[]).is_ok());
}

#[test]
fn guard_scale_bounds_are_two_to_the_minus_16_and_the_8() {
    let power = |k: i32| f32::from_bits(u32::try_from(127 + k).expect("a biased exponent") << 23);
    for k in -16..=8 {
        let grid = MapGrid::from_heightmap(1, 1, &[1, 1, 1, 1], power(k))
            .unwrap_or_else(|e| panic!("2^{k} is refused: {e}"));
        assert_eq!(grid.heights[0].to_bits(), 1_i64 << (32 + k), "2^{k}");
    }
    for k in [-17, 9] {
        assert_eq!(
            MapGrid::from_heightmap(1, 1, &[1, 1, 1, 1], power(k)),
            Err(Error::Ingest(IngestError::UnsupportedScale(power(k)))),
            "2^{k}"
        );
    }
}

// ---- SIM-048: the grid's edges ---------------------------------------------

#[test]
fn guard_grid_queries_outside_the_cells_are_false_or_zero() {
    let (width, height) = (4_u32, 3_u32);
    let samples: Vec<i16> = (0..20)
        .map(|i| i16::try_from(i * 3 + 1).expect("small"))
        .collect();
    let grid =
        MapGrid::from_heightmap(width, height, &samples, 0.007_812_5).expect("a 4x3 heightmap");
    let layers = [
        Layer::Air,
        Layer::Land,
        Layer::Orbit,
        Layer::Seabed,
        Layer::Sub,
        Layer::Water,
    ];
    for layer in layers {
        for (x, z) in [
            (width, 0),
            (0, height),
            (width, height),
            (u32::MAX, u32::MAX),
        ] {
            assert!(!grid.passable(layer, x, z), "{layer:?} at ({x}, {z})");
        }
    }
    assert!(
        grid.passable(Layer::Air, width - 1, height - 1),
        "inside is passable"
    );
    for (x, z) in [(width, 0), (0, height), (u32::MAX, u32::MAX)] {
        assert_eq!(grid.slope(x, z), Q::ZERO, "slope at ({x}, {z})");
    }
    for (x, z) in [(width + 1, 0), (0, height + 1), (u32::MAX, u32::MAX)] {
        assert_eq!(grid.height_at(x, z), Q::ZERO, "height at ({x}, {z})");
    }
    // The samples run to width and height inclusive.
    assert_ne!(grid.height_at(width, 0), Q::ZERO);
    assert_ne!(grid.height_at(0, height), Q::ZERO);
}

// ---- SIM-051: the skirmish --------------------------------------------------

fn kind(id: &str, health: i64, max_health: i64) -> UnitKind {
    UnitKind {
        id: id.to_owned(),
        health: Q::from_num(health),
        max_health: Q::from_num(max_health),
        ..UnitKind::default()
    }
}

fn army_marker(slot: u8) -> Marker {
    Marker {
        name: format!("ARMY_{slot}"),
        kind: MarkerKind::Army(slot),
        pos: Pos {
            x: f64::from(slot),
            y: 0.0,
            z: 1.0,
        },
    }
}

fn world_with(units: Vec<UnitKind>, slots: u8) -> Arc<World> {
    Arc::new(World {
        units: units.into_iter().map(|k| (k.id.clone(), k)).collect(),
        markers: (1..=slots).map(army_marker).collect(),
        ..World::empty()
    })
}

#[test]
fn guard_commander_spawns_at_max_health() {
    let world = world_with(vec![kind("uel0001", 40, 500)], 1);
    let state = world.skirmish(1, &[Faction::UEF]).expect("a skirmish");
    let unit = state.entities.values().next().expect("the commander");
    assert_eq!(unit.health, Q::from_num(500));
    assert_eq!(unit.max_health, Q::from_num(500));
}

#[test]
fn guard_faction_count_is_bounded_by_the_u8_slots() {
    let world = world_with(vec![kind("uel0001", 1, 1)], 255);
    let full = vec![Faction::UEF; 255];
    let state = Arc::clone(&world)
        .skirmish(1, &full)
        .expect("255 factions fit the slots");
    assert_eq!(state.armies.len(), 255);

    let over = vec![Faction::UEF; 256];
    assert_eq!(
        world.skirmish(1, &over),
        Err(Error::Ingest(IngestError::TooManyFactions { count: 256 }))
    );
}

#[test]
fn guard_commander_ids_are_the_lower_case_catalog_keys() {
    for (faction, id) in [
        (Faction::UEF, "uel0001"),
        (Faction::Aeon, "ual0001"),
        (Faction::Cybran, "url0001"),
        (Faction::Seraphim, "xsl0001"),
    ] {
        assert_eq!(faction.commander(), id);
    }
}
