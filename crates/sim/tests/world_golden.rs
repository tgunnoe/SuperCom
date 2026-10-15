//! The content check of a resumed state and the pinned digests of a
//! synthetic world (docs/specs/determinism.md DET-007; sim.md SIM-049 to
//! SIM-051). All synthetic: no install, no file read.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::sync::Arc;

use supercom_data as data;
use supercom_sim::{
    Error, MapGrid, Marker, MarkerKind, Pos, REPLAY_FORMAT_VERSION, ReplayError, ReplayHeader,
    State, World,
};

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

fn weapon(label: &str, projectile: &str) -> data::Weapon {
    data::Weapon {
        label: Some(label.to_owned()),
        damage: 25.5,
        rate_of_fire: Some(0.5),
        max_radius: Some(30.0),
        projectile_id: Some(projectile.to_owned()),
        damage_radius: Some(1.25),
        damage_type: "Normal".to_owned(),
        damage_friendly: Some(true),
        min_radius: None,
        muzzle_velocity: Some(12.0),
        projectiles_per_on_fire: None,
        muzzle_salvo_size: None,
        muzzle_salvo_delay: None,
        rack_salvo_reload_time: None,
        rack_salvo_charge_time: None,
        rack_salvo_fires_after_charge: None,
        ballistic_arc: None,
        target_priorities: Vec::new(),
        fire_target_layer_caps: vec![
            ("Land".to_owned(), "Land|Water".to_owned()),
            ("Air".to_owned(), "Air".to_owned()),
        ],
        tracking_radius: None,
        firing_tolerance: None,
        target_check_interval: None,
        weapon_category: "Direct Fire".to_owned(),
        turreted: Some(true),
        manual_fire: None,
        enabled_by_enhancement: None,
    }
}

fn unit(id: &str, mass: f64, weapons: Vec<data::Weapon>, builds: &[&str]) -> data::Unit {
    data::Unit {
        id: id.to_owned(),
        categories: strings(&["MOBILE", "LAND", "TECH1"]),
        economy: data::Economy {
            build_cost_energy: 120.25,
            build_cost_mass: mass,
            build_time: Some(300.0),
            build_rate: None,
            production_per_second_energy: None,
            production_per_second_mass: Some(0.1),
            storage_energy: None,
            storage_mass: Some(650.0),
            buildable_category: strings(builds),
            maintenance_consumption_per_second_energy: None,
        },
        physics: data::Physics {
            motion_type: "RULEUMT_Land".to_owned(),
            max_speed: Some(3.5),
            turn_rate: Some(90.0),
            max_acceleration: None,
            max_brake: None,
            elevation: None,
            min_speed_percent: None,
            build_restriction: Some("RULEUBR_OnMassDeposit".to_owned()),
            build_on_layer_caps: strings(&["LAYER_Land"]),
        },
        defense: data::Defense {
            health: 200.0,
            max_health: 200.0,
            armor_type: "Normal".to_owned(),
            regen_rate: Some(0.5),
        },
        intel: Some(data::Intel {
            vision_radius: Some(20.0),
            radar_radius: None,
        }),
        weapons,
        footprint: Some(data::Footprint {
            size_x: Some(2.0),
            size_z: Some(3.0),
            min_water_depth: None,
        }),
        command_caps: strings(&["RULEUCC_Attack", "RULEUCC_Move", "RULEUCC_Stop"]),
    }
}

fn projectile() -> data::Projectile {
    data::Projectile {
        id: "PROJ1".to_owned(),
        categories: strings(&["PROJECTILE", "DIRECTFIRE"]),
        physics: data::ProjectilePhysics {
            initial_speed: Some(2.0),
            max_speed: Some(40.0),
            acceleration: None,
            turn_rate: None,
            lifetime: Some(5.5),
            track_target: Some(true),
            use_gravity: None,
            lead_target: Some(true),
            velocity_align: None,
            destroy_on_water: Some(false),
        },
    }
}

/// A world with two unit kinds (one weapon, two weapons; the first builds
/// the second), one projectile, a 4x4-cell heightmap, and three markers of
/// different kinds, built through `World::from_parts` so the conversion is
/// inside the pinned value.
fn synthetic_world() -> World {
    let samples: Vec<i16> = (0..25).map(|i| (i % 7) * 20 - 30).collect();
    let map = MapGrid::from_heightmap(4, 4, &samples, 0.007_812_5).expect("a 4x4 heightmap");
    let markers = vec![
        Marker {
            name: "Mass 1".to_owned(),
            kind: MarkerKind::Mass,
            pos: Pos {
                x: 1.5,
                y: 0.25,
                z: 2.5,
            },
        },
        Marker {
            name: "ARMY_1".to_owned(),
            kind: MarkerKind::Army(1),
            pos: Pos {
                x: 0.5,
                y: 0.5,
                z: 3.5,
            },
        },
        Marker {
            name: "Rally".to_owned(),
            kind: MarkerKind::Other,
            pos: Pos {
                x: 3.0,
                y: 0.0,
                z: 3.0,
            },
        },
    ];
    World::from_parts(
        &[
            unit(
                "AAA0001",
                10.1,
                vec![weapon("Gun", "PROJ1")],
                &["MOBILE TECH1"],
            ),
            unit(
                "BBB0002",
                20.0,
                vec![weapon("Gun", "PROJ1"), weapon("Cannon", "PROJ1")],
                &[],
            ),
        ],
        &[projectile()],
        map,
        markers,
        "SYNTH".to_owned(),
    )
    .expect("a valid synthetic world")
}

fn header_for(world: &World) -> ReplayHeader {
    ReplayHeader {
        format_version: REPLAY_FORMAT_VERSION,
        engine_version: "test".to_owned(),
        map: world.map_name.clone(),
        map_digest: world.map_digest,
        catalog_digest: world.catalog_digest,
        plugins: Vec::new(),
        players: Vec::new(),
        seed: 5,
    }
}

#[test]
fn sim_state_with_world_checks_replay_content() {
    let world = synthetic_world();
    let state = State::with_world(Arc::new(world.clone()), 9);
    assert_eq!(**state.world(), world);
    assert_eq!(state.seed, 9);
    assert_eq!(state.tick.0, 0);
    assert!(state.armies.is_empty());
    assert!(state.entities.is_empty());

    let header = header_for(&world);
    assert_eq!(world.check_replay(&header), Ok(()));

    let mut wrong_catalog = header.clone();
    wrong_catalog.catalog_digest[31] ^= 1;
    assert_eq!(
        world.check_replay(&wrong_catalog),
        Err(Error::Replay(ReplayError::ContentMismatch))
    );
    let mut wrong_map = header;
    wrong_map.map_digest[0] ^= 1;
    assert_eq!(
        world.check_replay(&wrong_map),
        Err(Error::Replay(ReplayError::ContentMismatch))
    );
}

/// The digests of `synthetic_world`, pinned as literal bytes. They were taken
/// once from the implementation (a pinned value cannot be written before the
/// code that computes it); a re-run on another architecture is their check.
const CATALOG_DIGEST: [u8; 32] = [
    0xbc, 0x06, 0x36, 0x41, 0x02, 0x49, 0x61, 0xbb, 0x5d, 0xc4, 0x3b, 0x13, 0x37, 0xc3, 0x34, 0x75,
    0xf4, 0x18, 0xa6, 0x08, 0x39, 0x89, 0xc1, 0x96, 0x8d, 0x18, 0x27, 0x13, 0x2c, 0x93, 0x5e, 0x77,
];
const MAP_DIGEST: [u8; 32] = [
    0x6d, 0x79, 0x0e, 0xe5, 0xd3, 0xdb, 0x6c, 0x7f, 0x78, 0x18, 0x42, 0x1c, 0x2c, 0x8e, 0xb8, 0x51,
    0x8e, 0xc4, 0x70, 0x7c, 0x0a, 0x55, 0xaf, 0x65, 0x05, 0xdc, 0x4b, 0xa0, 0x66, 0x16, 0x40, 0x61,
];

#[test]
fn sim_world_digests_golden_vector() {
    let world = synthetic_world();
    assert_eq!(world.catalog_digest, CATALOG_DIGEST, "catalog digest");
    assert_eq!(world.map_digest, MAP_DIGEST, "map digest");
    let again = synthetic_world();
    assert_eq!(world.catalog_digest, again.catalog_digest);
    assert_eq!(world.map_digest, again.map_digest);
}
