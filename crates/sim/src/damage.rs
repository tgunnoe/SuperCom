//! The damage and death phases (SIM-078 to SIM-080): impact damage through
//! the armour table, regeneration, and the removal of the dead.

use std::sync::Arc;

use crate::build::{kind_of, total_work};
use crate::combat::{Impact, Scratch, distance_xz};
use crate::command::Order;
use crate::economy::per_tick;
use crate::ids::{EntityId, Q};
use crate::state::State;
use crate::world::World;

impl State {
    /// The damage phase: each impact in projectile-id order hurts the entity
    /// it landed on and, with a radius, every other enemy inside it; then
    /// every hurt complete entity regenerates (SIM-078, SIM-079).
    pub(crate) fn run_damage(&mut self, scratch: &mut Scratch) {
        let world = Arc::clone(self.world());
        for impact in std::mem::take(&mut scratch.impacts) {
            self.counters.projectile_impacts = self.counters.projectile_impacts.saturating_add(1);
            for id in self.hit_by(&impact) {
                self.hurt(&world, id, &impact, scratch);
            }
        }
        self.regenerate(&world);
    }

    /// The entities an impact reaches, the one it landed on first.
    fn hit_by(&self, impact: &Impact) -> Vec<EntityId> {
        let mut hit: Vec<EntityId> = impact
            .on
            .filter(|id| self.entities.contains_key(id))
            .into_iter()
            .collect();
        if impact.damage_radius > 0.0 {
            hit.extend(
                self.entities
                    .iter()
                    .filter(|(id, e)| {
                        Some(**id) != impact.on
                            && e.army != impact.army
                            && distance_xz(&e.pos, &impact.pos) <= impact.damage_radius
                    })
                    .map(|(id, _)| *id),
            );
        }
        hit
    }

    /// Apply an impact's damage, times the armour multiplier, to one entity.
    fn hurt(&mut self, world: &World, id: EntityId, impact: &Impact, scratch: &mut Scratch) {
        let Some(e) = self.entities.get_mut(&id) else {
            return;
        };
        let kind = kind_of(world, &e.blueprint);
        let multiplier = kind.map_or(Q::ONE, |k| {
            world.armour_multiplier(&k.armor_type, &impact.damage_type)
        });
        let amount = impact.damage.saturating_mul(multiplier);
        if e.complete {
            e.health = e.health.saturating_sub(amount);
            return;
        }
        // An unfinished entity loses built work in proportion to its health.
        let Some(kind) = kind else {
            return;
        };
        let lost = amount
            .saturating_mul(total_work(kind))
            .checked_div(e.max_health)
            .unwrap_or(Q::ZERO);
        e.built = e.built.saturating_sub(lost).max(Q::ZERO);
        e.health = e.max_health.saturating_mul(e.progress(kind));
        if amount > Q::ZERO && e.built == Q::ZERO {
            scratch.marked.insert(id);
        }
    }

    /// Regeneration never heals an entity at or below zero health: the
    /// death phase removes it this tick (SIM-079).
    fn regenerate(&mut self, world: &World) {
        for e in self
            .entities
            .values_mut()
            .filter(|e| e.complete && e.health > Q::ZERO)
        {
            let rate = kind_of(world, &e.blueprint)
                .and_then(|k| k.regen_rate)
                .filter(|r| *r > Q::ZERO);
            if let Some(rate) = rate.filter(|_| e.health < e.max_health) {
                e.health = e.health.saturating_add(per_tick(rate)).min(e.max_health);
            }
        }
    }

    /// The death phase: a complete entity at or below zero health and an
    /// unfinished one the damage phase marked are removed in ascending id;
    /// an Attack at the head of another queue naming one is popped
    /// (SIM-080).
    pub(crate) fn run_death(&mut self, scratch: &Scratch) {
        let dead: Vec<EntityId> = self
            .entities
            .iter()
            .filter(|(id, e)| {
                if e.complete {
                    e.health <= Q::ZERO
                } else {
                    scratch.marked.contains(id)
                }
            })
            .map(|(id, _)| *id)
            .collect();
        for id in dead {
            self.entities.remove(&id);
            self.counters.deaths = self.counters.deaths.saturating_add(1);
            for e in self.entities.values_mut() {
                if matches!(e.orders.front(), Some(Order::Attack { target }) if *target == id) {
                    e.orders.pop_front();
                }
            }
        }
    }
}
