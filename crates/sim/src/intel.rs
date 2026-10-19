//! The intel phase: which entities each army sees (SIM-083).

use std::collections::{BTreeMap, BTreeSet};

use crate::combat::distance_xz;
use crate::ids::{Army, EntityId, Pos};
use crate::state::State;

impl State {
    /// Rebuild every army's visible set from the entities' positions: an
    /// army sees its own entities, and any other entity within the vision
    /// radius of one of its complete seeing entities, measured on the
    /// horizontal plane and inclusive (SIM-083). The sets are rebuilt from
    /// scratch, so nothing seen last tick persists unless it is still in
    /// range.
    pub(crate) fn run_intel(&mut self) {
        let seers: Vec<(Army, Pos, f64)> = self
            .entities
            .values()
            .filter(|e| e.complete)
            .filter_map(|e| {
                let radius = self.world().units.get(&e.blueprint)?.vision_radius?;
                Some((e.army, e.pos, radius))
            })
            .collect();
        let mut sets: BTreeMap<Army, BTreeSet<EntityId>> = BTreeMap::new();
        for army in self.armies.keys() {
            let seen = self
                .entities
                .values()
                .filter(|e| {
                    e.army == *army
                        || seers
                            .iter()
                            .any(|(a, p, r)| a == army && distance_xz(p, &e.pos) <= *r)
                })
                .map(|e| e.id)
                .collect();
            sets.insert(*army, seen);
        }
        self.visible = sets;
    }
}
