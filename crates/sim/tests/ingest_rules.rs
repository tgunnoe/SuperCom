//! Carry-forward checks on the ingest rules (docs/specs/sim.md, SIM-050):
//! ties round to even, and an explicit `RULEUBR_None` is no restriction.
//! Synthetic: no install, no file.

use supercom_data as data;
use supercom_sim::{BuildRestriction, World};

/// 2^-33, exactly.
const TWO_POW_MINUS_33: f64 = 1.0 / 8_589_934_592.0;

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

fn typed_unit(id: &str, cost: f64) -> data::Unit {
    data::Unit {
        id: id.to_owned(),
        categories: strings(&["STRUCTURE"]),
        economy: data::Economy {
            build_cost_energy: 1.0,
            build_cost_mass: cost,
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
            motion_type: "RULEUMT_None".to_owned(),
            max_speed: None,
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

/// SIM-050: a value exactly halfway between two representable numbers
/// converts to the even one (2^-33 to 0, 3 * 2^-33 to 2 raw units), and an
/// explicit `RULEUBR_None` reads as no restriction.
#[test]
fn sim_ingest_rounds_ties_to_even_and_reads_rulebr_none() {
    let units = [
        typed_unit("HALF", TWO_POW_MINUS_33),
        typed_unit("THREE", 3.0 * TWO_POW_MINUS_33),
    ];
    let world = World::from_parts(
        &units,
        &[],
        World::empty().map,
        Vec::new(),
        "tie".to_owned(),
    )
    .expect("values inside the range convert");
    let raw = |id: &str| world.units.get(id).map(|k| k.build_cost_mass.to_bits());
    assert_eq!(raw("half"), Some(0));
    assert_eq!(raw("three"), Some(2));

    assert_eq!(
        BuildRestriction::from_rule(Some("RULEUBR_None")).ok(),
        Some(BuildRestriction::None)
    );
}
