//! The match outcome (docs/adr/0014-vision-and-victory.md, SIM-084,
//! SIM-085).

use std::collections::BTreeSet;

use crate::ids::Army;
use crate::state::State;

/// The category that keeps an army in the match under the demoralization
/// rule (SIM-084).
const COMMAND: &str = "COMMAND";

/// How a match ended: one army is the last standing, or none is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Winner(Army),
    Draw,
}

impl State {
    /// Defeat every non-civilian army without a complete commander, then name
    /// the outcome (SIM-084, SIM-085), while the match is undecided and at
    /// least two non-civilian armies exist. Once an outcome exists the phase
    /// does nothing, so the outcome, the defeat marks, and the `defeats` and
    /// `victories` counters never change again; `kills` and `deaths` are not
    /// among them (SIM-087).
    pub(crate) fn run_victory(&mut self) {
        // A lone army is not defeated either: a match needs an opponent
        // before it can be lost (SIM-085).
        if self.outcome.is_some() || self.armies.values().filter(|a| !a.civilian).count() < 2 {
            return;
        }
        // No commander can exist in a world without a COMMAND kind, so the
        // rule would defeat every army at once (SIM-084).
        if !self
            .world()
            .units
            .values()
            .any(|k| k.categories.contains(COMMAND))
        {
            return;
        }
        let commanded: BTreeSet<Army> = self
            .entities
            .values()
            .filter(|e| {
                e.complete
                    && self
                        .world()
                        .units
                        .get(&e.blueprint)
                        .is_some_and(|k| k.categories.contains(COMMAND))
            })
            .map(|e| e.army)
            .collect();
        let tick = self.tick;
        for (slot, army) in &mut self.armies {
            if !army.civilian && army.defeated.is_none() && !commanded.contains(slot) {
                army.defeated = Some(tick);
                self.counters.defeats = self.counters.defeats.saturating_add(1);
            }
        }
        let mut standing = self
            .armies
            .values()
            .filter(|a| !a.civilian && a.defeated.is_none());
        self.outcome = match (standing.next(), standing.next()) {
            (None, _) => Some(Outcome::Draw),
            (Some(only), None) => {
                self.counters.victories = self.counters.victories.saturating_add(1);
                Some(Outcome::Winner(only.id))
            }
            (Some(_), Some(_)) => None,
        };
    }
}
