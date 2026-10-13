//! Acceptance tests of the sim's static world (docs/specs/sim.md, SIM-048 to
//! SIM-051). All synthetic: no install, no archive, no file read. Each test is
//! cited by a clause; the installed-data checks live in
//! `crates/sim-ingest/tests/conformance.rs`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use supercom_data as data;
use supercom_sim::{
    Army, BuildRestriction, Error, Faction, IngestError, Layer, LayerSet, MapGrid, Marker,
    MarkerKind, MotionType, Order, OrderCaps, OrderKind, Pos, Q, UnitKind, World,
};

// ---- clause SIM-049: closed sets ----------------------------------------

/// Every `OrderKind` with its key, in `Ord` order (SIM-016).
const CAPABILITIES: [(&str, OrderKind); 21] = [
    ("RULEUCC_Attack", OrderKind::Attack),
    ("RULEUCC_CallTransport", OrderKind::CallTransport),
    ("RULEUCC_Capture", OrderKind::Capture),
    ("RULEUCC_Dive", OrderKind::Dive),
    ("RULEUCC_Dock", OrderKind::Dock),
    ("RULEUCC_Ferry", OrderKind::Ferry),
    ("RULEUCC_Guard", OrderKind::Guard),
    ("RULEUCC_Move", OrderKind::Move),
    ("RULEUCC_Nuke", OrderKind::Nuke),
    ("RULEUCC_Overcharge", OrderKind::Overcharge),
    ("RULEUCC_Patrol", OrderKind::Patrol),
    ("RULEUCC_Pause", OrderKind::Pause),
    ("RULEUCC_Reclaim", OrderKind::Reclaim),
    ("RULEUCC_Repair", OrderKind::Repair),
    ("RULEUCC_RetaliateToggle", OrderKind::RetaliateToggle),
    ("RULEUCC_Sacrifice", OrderKind::Sacrifice),
    ("RULEUCC_SiloBuildNuke", OrderKind::SiloBuildNuke),
    ("RULEUCC_SiloBuildTactical", OrderKind::SiloBuildTactical),
    ("RULEUCC_Stop", OrderKind::Stop),
    ("RULEUCC_Tactical", OrderKind::Tactical),
    ("RULEUCC_Transport", OrderKind::Transport),
];

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn sim_layer_set_and_caps_parse() {
    // The six layer keys, ascending.
    let layers = [
        ("LAYER_Air", Layer::Air),
        ("LAYER_Land", Layer::Land),
        ("LAYER_Orbit", Layer::Orbit),
        ("LAYER_Seabed", Layer::Seabed),
        ("LAYER_Sub", Layer::Sub),
        ("LAYER_Water", Layer::Water),
    ];
    for (key, layer) in layers {
        assert_eq!(Layer::from_key(key), Some(layer), "{key}");
    }
    assert!(layers.windows(2).all(|w| w[0].1 < w[1].1));
    assert_eq!(Layer::from_key("LAYER_Bogus"), None);

    // A caps string such as FireTargetLayerCapsTable holds.
    let caps = LayerSet::from_caps("Land|Water|Seabed").expect("a valid caps string");
    for layer in [Layer::Land, Layer::Water, Layer::Seabed] {
        assert!(caps.contains(layer), "{layer:?}");
    }
    for layer in [Layer::Air, Layer::Orbit, Layer::Sub] {
        assert!(!caps.contains(layer), "{layer:?}");
    }
    assert_eq!(
        caps.iter().collect::<Vec<_>>(),
        [Layer::Land, Layer::Seabed, Layer::Water]
    );
    assert_eq!(
        LayerSet::from_caps("Land|Bogus"),
        Err(Error::Ingest(IngestError::UnknownLayerKey("Bogus".into())))
    );

    // BuildOnLayerCaps keys.
    let keys = LayerSet::from_keys(&strings(&["LAYER_Air", "LAYER_Sub"])).expect("valid keys");
    assert_eq!(keys.iter().collect::<Vec<_>>(), [Layer::Air, Layer::Sub]);
    assert_eq!(
        LayerSet::from_keys(&strings(&["LAYER_Land", "LAYER_Bogus"])),
        Err(Error::Ingest(IngestError::UnknownLayerKey(
            "LAYER_Bogus".into()
        )))
    );

    // The 21 command capabilities.
    for (key, kind) in CAPABILITIES {
        assert_eq!(OrderKind::from_key(key), Some(kind), "{key}");
    }
    assert!(CAPABILITIES.windows(2).all(|w| w[0].1 < w[1].1));
    assert_eq!(OrderKind::from_key("RULEUCC_Bogus"), None);
    let granted = OrderCaps::from_keys(&strings(&["RULEUCC_Move", "RULEUCC_Stop"]))
        .expect("valid capabilities");
    for (_, kind) in CAPABILITIES {
        assert_eq!(
            granted.allows(kind),
            matches!(kind, OrderKind::Move | OrderKind::Stop),
            "{kind:?}"
        );
    }
    assert_eq!(
        OrderCaps::from_keys(&strings(&["RULEUCC_Move", "RULEUCC_Bogus"])),
        Err(Error::Ingest(IngestError::UnknownCapability(
            "RULEUCC_Bogus".into()
        )))
    );

    check_order_capabilities();

    check_motion_and_restrictions();
}

fn check_order_capabilities() {
    // Orders and the capability that gates them; building is gated by the
    // builder's buildable set, not by a capability.
    let pos = Pos {
        x: 1.0,
        y: 0.0,
        z: 1.0,
    };
    assert_eq!(Order::Move { to: pos }.capability(), Some(OrderKind::Move));
    assert_eq!(Order::Stop.capability(), Some(OrderKind::Stop));
    assert_eq!(
        Order::Attack {
            target: supercom_sim::EntityId(1)
        }
        .capability(),
        Some(OrderKind::Attack)
    );
    assert_eq!(
        Order::Guard {
            target: supercom_sim::EntityId(1)
        }
        .capability(),
        Some(OrderKind::Guard)
    );
    assert_eq!(
        Order::BuildMobile {
            blueprint: "UEL0101".into(),
            at: pos
        }
        .capability(),
        None
    );
    assert_eq!(
        Order::BuildFactory {
            blueprint: "UEL0101".into()
        }
        .capability(),
        None
    );
}

fn check_motion_and_restrictions() {
    // The eight motion types.
    let motions = [
        ("RULEUMT_None", MotionType::None),
        ("RULEUMT_Air", MotionType::Air),
        ("RULEUMT_Land", MotionType::Land),
        ("RULEUMT_Water", MotionType::Water),
        ("RULEUMT_Hover", MotionType::Hover),
        ("RULEUMT_Amphibious", MotionType::Amphibious),
        ("RULEUMT_SurfacingSub", MotionType::SurfacingSub),
        ("RULEUMT_AmphibiousFloating", MotionType::AmphibiousFloating),
    ];
    for (rule, motion) in motions {
        assert_eq!(MotionType::from_rule(rule), Some(motion), "{rule}");
    }
    assert_eq!(MotionType::from_rule("RULEUMT_Bogus"), None);

    // Build restrictions: an absent field is None, an unknown value an error.
    assert_eq!(
        BuildRestriction::from_rule(None),
        Ok(BuildRestriction::None)
    );
    assert_eq!(
        BuildRestriction::from_rule(Some("RULEUBR_OnMassDeposit")),
        Ok(BuildRestriction::OnMassDeposit)
    );
    assert_eq!(
        BuildRestriction::from_rule(Some("RULEUBR_OnHydrocarbonDeposit")),
        Ok(BuildRestriction::OnHydrocarbonDeposit)
    );
    assert_eq!(
        BuildRestriction::from_rule(Some("RULEUBR_Bogus")),
        Err(Error::Ingest(IngestError::UnknownBuildRestriction(
            "RULEUBR_Bogus".into()
        )))
    );
}

// ---- clause SIM-050: rounding and overflow --------------------------------

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

fn from_units(units: &[data::Unit]) -> Result<World, Error> {
    World::from_parts(
        units,
        &[],
        World::empty().map,
        Vec::new(),
        "synthetic".to_owned(),
    )
}

#[test]
fn sim_ingest_rounds_to_nearest_q_and_refuses_overflow() {
    // 0.1 is not representable; the nearest I32F32 is 429496730 / 2^32
    // (0.1 * 2^32 = 429496729.6), not the truncated 429496729.
    let mut tenth = typed_unit("TENTH");
    tenth.economy.build_cost_mass = 0.1;
    tenth.economy.build_cost_energy = 2_147_483_647.0;
    tenth.economy.storage_mass = Some(0.1);
    let world = from_units(&[tenth]).expect("values inside the range convert");
    let kind = world.units.get("tenth").expect("the unit is in the world");
    assert_eq!(kind.build_cost_mass.to_bits(), 429_496_730);
    assert_eq!(kind.storage_mass.map(Q::to_bits), Some(429_496_730));
    assert_eq!(kind.build_cost_energy.to_bits(), 2_147_483_647_i64 << 32);

    // 2^31 and beyond are refused with the field's path, never wrapped or
    // saturated; so is a value that is not finite.
    let mut over = typed_unit("OVER");
    over.economy.build_cost_mass = 2_147_483_648.0;
    let mut stored = typed_unit("STORED");
    stored.economy.storage_mass = Some(-2_147_483_649.0);
    let mut nan = typed_unit("NAN");
    nan.defense.health = f64::NAN;
    for (unit, field) in [
        (over, "build_cost_mass"),
        (stored, "storage_mass"),
        (nan, "health"),
    ] {
        let id = unit.id.clone();
        match from_units(&[unit]) {
            Err(Error::Ingest(IngestError::ValueOverflow { path })) => {
                assert!(path.contains(&id), "{path} names the unit {id}");
                assert!(path.contains(field), "{path} names the field {field}");
            }
            other => panic!("{id}: expected ValueOverflow, got {other:?}"),
        }
    }
}

// ---- clause SIM-048: land passability from the heightmap -------------------

const SCALE: f32 = 0.007_812_5; // 2^-7, the scale of every installed map

/// Column heights (raw `i16` samples) of the first row of cells: flat, flat,
/// ramp 0.5, ramp 0.5, edge exactly 0.75 (96), just over (97), cliff (400),
/// flat, flat.
const COLUMNS: [i16; 9] = [0, 0, 64, 128, 224, 321, 721, 721, 721];

/// The 8x8-cell test map. Rows 0..=3 repeat `COLUMNS`; rows 4..=7 climb 64
/// per row on top of it (a plane, so cells on a ramp have a corner pair on
/// the diagonal that differs by more than 0.75 while every edge stays under
/// it); the last row adds a 200 step, a vertical cliff under cells whose
/// horizontal edges are flat.
fn samples() -> Vec<i16> {
    let mut out = Vec::new();
    for z in 0..9_i16 {
        for column in COLUMNS {
            let climb = if z >= 4 { 64 * (z - 4) } else { 0 };
            let step = if z == 8 { 200 } else { 0 };
            out.push(column + climb + step);
        }
    }
    out
}

/// The test's own slope rule: the largest absolute difference between
/// adjacent corners over the four edges of the cell, in raw units.
fn reference_slope(samples: &[i16], x: usize, z: usize) -> i32 {
    let at = |cx: usize, cz: usize| i32::from(samples[cz * 9 + cx]);
    let corners = [at(x, z), at(x + 1, z), at(x + 1, z + 1), at(x, z + 1)];
    (0..4)
        .map(|i| (corners[i] - corners[(i + 1) % 4]).abs())
        .max()
        .expect("four edges")
}

#[test]
fn sim_land_passability_from_heightmap() {
    let samples = samples();
    let grid = MapGrid::from_heightmap(8, 8, &samples, SCALE).expect("a valid 8x8 heightmap");
    assert_eq!((grid.width, grid.height), (8, 8));
    assert_eq!(grid.heights.len(), 81);
    assert_eq!(
        grid.heights[2].to_bits(),
        i64::from(samples[2]) << 25,
        "a sample converts exactly: raw * 2^-7 is raw << 25 in I32F32"
    );

    // Hand-checked cells: flat, ramp, exactly 0.75, just over, cliff, and the
    // vertical cliff of the last row.
    assert_eq!(grid.slope(0, 0), Q::ZERO);
    assert_eq!(grid.slope(1, 0), Q::from_num(0.5));
    assert_eq!(grid.slope(3, 0), Q::from_num(0.75));
    assert_eq!(grid.slope(4, 0), Q::from_num(97) / Q::from_num(128));
    assert_eq!(grid.slope(6, 7), Q::from_num(264) / Q::from_num(128));

    for z in 0..8_u32 {
        for x in 0..8_u32 {
            let slope = reference_slope(&samples, x as usize, z as usize);
            let want = slope <= 96;
            assert_eq!(
                grid.passable(Layer::Land, x, z),
                want,
                "land cell ({x}, {z}) with raw slope {slope}"
            );
            assert!(grid.passable(Layer::Air, x, z), "air cell ({x}, {z})");
            for absent in [Layer::Water, Layer::Seabed, Layer::Sub] {
                assert!(
                    !grid.passable(absent, x, z),
                    "{absent:?} is absent, so false at ({x}, {z})"
                );
            }
        }
    }
    // The pattern the construction fixes: columns 0..=3 and 6..=7 pass in
    // rows 0..=6 (the planar cells in columns 1 and 2 pass although their
    // diagonal differs by 1.0), the last row passes nowhere.
    let land = &grid.passable[&Layer::Land];
    let passable: BTreeSet<(u32, u32)> = (0..64_u32)
        .filter(|i| land[*i as usize])
        .map(|i| (i % 8, i / 8))
        .collect();
    let expected: BTreeSet<(u32, u32)> = (0..7_u32)
        .flat_map(|z| [0, 1, 2, 3, 6, 7].map(|x| (x, z)))
        .collect();
    assert_eq!(passable, expected);
    assert!(!grid.passable.contains_key(&Layer::Water));

    // Scales the grid cannot hold exactly are refused.
    assert_eq!(
        MapGrid::from_heightmap(8, 8, &samples, 0.1),
        Err(Error::Ingest(IngestError::UnsupportedScale(0.1)))
    );
    assert_eq!(
        MapGrid::from_heightmap(8, 8, &samples[..80], SCALE),
        Err(Error::Ingest(IngestError::HeightmapSize {
            expected: 81,
            found: 80
        }))
    );
}

// ---- clause SIM-051: the skirmish starts full ------------------------------

fn commander(id: &str, mass: i64, energy: i64, health: i64) -> UnitKind {
    UnitKind {
        id: id.to_owned(),
        storage_mass: Some(Q::from_num(mass)),
        storage_energy: Some(Q::from_num(energy)),
        health: Q::from_num(health),
        max_health: Q::from_num(health),
        ..UnitKind::default()
    }
}

fn marker(name: &str, kind: MarkerKind, x: f64, z: f64) -> Marker {
    Marker {
        name: name.to_owned(),
        kind,
        pos: Pos { x, y: 6.75, z },
    }
}

/// A 16x16 synthetic world with two commanders whose storage values differ,
/// so an army fed another army's values is caught.
fn two_army_world() -> Arc<World> {
    let units: BTreeMap<String, UnitKind> = [
        commander("uel0001", 650, 4000, 5000),
        commander("url0001", 700, 4100, 6000),
    ]
    .into_iter()
    .map(|k| (k.id.clone(), k))
    .collect();
    Arc::new(World {
        units,
        map: MapGrid {
            width: 16,
            height: 16,
            heights: vec![Q::ZERO; 17 * 17],
            passable: BTreeMap::new(),
        },
        markers: vec![
            marker("ARMY_1", MarkerKind::Army(1), 2.5, 13.5),
            marker("ARMY_2", MarkerKind::Army(2), 13.5, 2.5),
            marker("Mass 1", MarkerKind::Mass, 8.0, 8.0),
        ],
        map_name: "synthetic".to_owned(),
        ..World::empty()
    })
}

#[test]
fn sim_skirmish_places_commanders_and_starts_full() {
    let world = two_army_world();
    let state = Arc::clone(&world)
        .skirmish(7, &[Faction::UEF, Faction::Cybran])
        .expect("a skirmish on a world with both commanders and markers");
    assert!(Arc::ptr_eq(state.world(), &world));
    assert_eq!(state.seed, 7);

    assert_eq!(
        state.armies.keys().copied().collect::<Vec<_>>(),
        [Army(1), Army(2)]
    );
    for (slot, id, mass, energy, health, x, z) in [
        (1, "uel0001", 650, 4000, 5000, 2.5, 13.5),
        (2, "url0001", 700, 4100, 6000, 13.5, 2.5),
    ] {
        let army = &state.armies[&Army(slot)];
        assert_eq!(army.mass, Q::from_num(mass), "army {slot} mass");
        assert_eq!(army.energy, Q::from_num(energy), "army {slot} energy");
        assert_eq!(army.mass_storage, Q::from_num(mass), "army {slot} storage");
        assert_eq!(army.energy_storage, Q::from_num(energy));
        assert!(!army.defeated);
        let own: Vec<_> = state
            .entities
            .values()
            .filter(|e| e.army == Army(slot))
            .collect();
        assert_eq!(own.len(), 1, "army {slot} has one unit");
        let unit = own[0];
        assert_eq!(unit.blueprint, id);
        assert_eq!(unit.health, Q::from_num(health));
        assert_eq!(unit.max_health, Q::from_num(health));
        assert_eq!((unit.pos.x, unit.pos.y, unit.pos.z), (x, 6.75, z));
    }
    assert_eq!(state.entities.len(), 2);

    // Two constructions agree, and so does the first empty-batch step.
    let mut a = Arc::clone(&world)
        .skirmish(7, &[Faction::UEF, Faction::Cybran])
        .expect("first construction");
    let mut b = Arc::clone(&world)
        .skirmish(7, &[Faction::UEF, Faction::Cybran])
        .expect("second construction");
    assert_eq!(a, b);
    assert_eq!(
        a.step(&[]).expect("first step"),
        b.step(&[]).expect("first step")
    );

    // A slot with no marker, and a commander the world lacks, are errors.
    assert_eq!(
        Arc::clone(&world).skirmish(7, &[Faction::UEF, Faction::Cybran, Faction::UEF]),
        Err(Error::Ingest(IngestError::MissingArmyMarker(3)))
    );
    assert_eq!(
        Arc::clone(&world).skirmish(7, &[Faction::Seraphim]),
        Err(Error::Ingest(IngestError::UnknownCommander(
            "xsl0001".into()
        )))
    );

    // A state built without a world holds the empty one.
    assert_eq!(**supercom_sim::State::new(1).world(), World::empty());
}
