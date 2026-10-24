//! The weapons phase (SIM-075, SIM-076): the Attack order's validity,
//! target acquisition, the firing clock, and the launch of a shot.

use std::sync::Arc;

use crate::build::kind_of;
use crate::combat::{GRAVITY, Projectile, ProjectileId, WeaponState, cooldown_ticks, distance_xz};
use crate::command::Order;
use crate::ids::{Army, EntityId, Pos};
use crate::state::{Entity, State};
use crate::world::{UnitKind, WeaponKind, World};

/// Whether a weapon acts at all: not a death or kamikaze weapon, not manual
/// fire, not enabled by an enhancement, and with a rate of fire (SIM-075).
pub(crate) fn is_active(weapon: &WeaponKind) -> bool {
    weapon.category != "Death"
        && weapon.category != "Kamikaze"
        && !weapon.manual_fire
        && !weapon.enabled_by_enhancement
        && weapon.rate_of_fire.is_some()
}

/// The largest maximum radius among a kind's active weapons; `None` when
/// none of them has one (SIM-081).
pub(crate) fn attack_range(kind: &UnitKind) -> Option<f64> {
    kind.weapons
        .iter()
        .filter(|w| is_active(w))
        .filter_map(|w| w.max_radius)
        .max_by(f64::total_cmp)
}

/// The distance below which a mobile attacker cannot fire and backs off
/// (SIM-086): the smallest positive minimum radius among the active weapons
/// that carry a maximum radius. `None` when one of those weapons has no
/// positive minimum (it can fire from anywhere), when none has a maximum
/// radius, or when the smallest minimum is not below [`attack_range`] (such
/// a kind could never fire, so no floor is honoured).
pub(crate) fn band_floor(kind: &UnitKind) -> Option<f64> {
    let mut floor: Option<f64> = None;
    for w in kind.weapons.iter().filter(|w| is_active(w)) {
        if w.max_radius.is_none() {
            continue;
        }
        let min = w.min_radius.filter(|m| *m > 0.0)?;
        floor = Some(floor.map_or(min, |f| f.min(min)));
    }
    floor.filter(|f| attack_range(kind).is_some_and(|range| *f < range))
}

/// Whether `weapon` fired from `from` may take `target`: inside the radii by
/// planar distance and on a layer the weapon allows (SIM-075).
fn can_hit(weapon: &WeaponKind, from: &Pos, target: &Entity) -> bool {
    let Some(max) = weapon.max_radius else {
        return false;
    };
    let d = distance_xz(from, &target.pos);
    d <= max
        && weapon.min_radius.is_none_or(|min| d >= min)
        && weapon.target_layers.contains(target.layer)
}

/// The shot a weapon makes at `target` (SIM-076, SIM-077): an instant hit
/// when the projectile kind or the speed is missing or the aim lies on the
/// firer or straight above or below it, else a projectile of the kind's
/// speed, elevated for a kind that falls.
fn shot(
    world: &World,
    firer: (&Entity, EntityId),
    weapon: &WeaponKind,
    target: &Entity,
    target_id: EntityId,
    id: ProjectileId,
) -> Projectile {
    let (from, source) = firer;
    let kind = weapon
        .projectile_id
        .as_deref()
        .and_then(|p| world.projectiles.get(p));
    let speed = weapon
        .muzzle_velocity
        .filter(|v| *v > 0.0)
        .or_else(|| kind.and_then(|k| k.initial_speed).filter(|v| *v > 0.0));
    let aim = target.pos;
    let (dx, dy, dz) = (aim.x - from.pos.x, aim.y - from.pos.y, aim.z - from.pos.z);
    let planar = distance_xz(&from.pos, &aim);
    let flight = kind
        .zip(speed)
        .filter(|_| planar > 0.0 && (dx, dy, dz) != (0.0, 0.0, 0.0));
    let (kind_id, vel, lifetime) = match flight {
        None => (None, [0.0; 3], None),
        Some((k, speed)) if k.use_gravity => {
            let reach = (-GRAVITY * planar / (speed * speed)).min(1.0);
            let elevation = 0.5 * libm::asin(reach);
            let flat = speed * libm::cos(elevation);
            (
                Some(k.id.clone()),
                [
                    flat * dx / planar,
                    speed * libm::sin(elevation),
                    flat * dz / planar,
                ],
                k.lifetime,
            )
        }
        Some((k, speed)) => {
            let len = libm::sqrt(dx * dx + dy * dy + dz * dz);
            (
                Some(k.id.clone()),
                [speed * dx / len, speed * dy / len, speed * dz / len],
                k.lifetime,
            )
        }
    };
    Projectile {
        id,
        army: from.army,
        source,
        kind: kind_id,
        pos: from.pos,
        vel,
        target: Some(target_id),
        aim,
        damage: weapon.damage,
        damage_radius: weapon.damage_radius.unwrap_or(0.0),
        damage_type: weapon.damage_type.clone(),
        lifetime,
    }
}

impl State {
    /// The weapons phase: each complete entity, in ascending id, judges its
    /// head Attack, then each of its weapons picks a target, counts its
    /// cooldown down, and fires (SIM-075, SIM-076).
    pub(crate) fn run_weapons(&mut self) {
        let world = Arc::clone(self.world());
        let ids: Vec<EntityId> = self.entities.keys().copied().collect();
        for id in ids {
            self.weapons_of(&world, id);
        }
    }

    /// The nearest enemy `weapon` may take from `pos`, ties to the lowest
    /// id; the head Attack's target first, then a target already held.
    fn choose_target(
        &self,
        army: Army,
        pos: &Pos,
        weapon: &WeaponKind,
        attack: Option<EntityId>,
        held: Option<EntityId>,
    ) -> Option<EntityId> {
        let hits = |id: EntityId| {
            self.entities
                .get(&id)
                .is_some_and(|t| t.army != army && can_hit(weapon, pos, t))
        };
        if let Some(t) = attack.filter(|t| hits(*t)) {
            return Some(t);
        }
        if let Some(h) = held.filter(|h| hits(*h)) {
            return Some(h);
        }
        self.entities
            .iter()
            .filter(|(_, t)| t.army != army && can_hit(weapon, pos, t))
            .fold(None, |best: Option<(f64, EntityId)>, (id, t)| {
                let d = distance_xz(pos, &t.pos);
                match best {
                    Some((nearest, _)) if nearest <= d => best,
                    _ => Some((d, *id)),
                }
            })
            .map(|(_, id)| id)
    }

    fn weapons_of(&mut self, world: &World, id: EntityId) {
        let Some(e) = self.entities.get(&id).filter(|e| e.complete) else {
            return;
        };
        let kind = kind_of(world, &e.blueprint);
        if let Some(Order::Attack { target }) = e.orders.front() {
            let enemy = self.entities.get(target).is_some_and(|t| t.army != e.army);
            if !(enemy && kind.and_then(attack_range).is_some()) {
                if let Some(e) = self.entities.get_mut(&id) {
                    e.orders.pop_front();
                }
                self.counters.commands_ignored = self.counters.commands_ignored.saturating_add(1);
            }
        }
        let (Some(kind), Some(e)) = (kind, self.entities.get(&id)) else {
            return;
        };
        let attack = match e.orders.front() {
            Some(Order::Attack { target }) => Some(*target),
            _ => None,
        };
        let mut updated = Vec::with_capacity(e.weapons.len());
        let mut shots = Vec::new();
        for (w, state) in kind.weapons.iter().zip(&e.weapons) {
            let target = is_active(w)
                .then(|| self.choose_target(e.army, &e.pos, w, attack, state.target))
                .flatten();
            let mut cooldown = state.cooldown.saturating_sub(1);
            if let Some(t) = target.filter(|_| cooldown == 0) {
                shots.push((w, t));
                cooldown = w.rate_of_fire.map_or(1, cooldown_ticks);
            }
            updated.push(WeaponState { target, cooldown });
        }
        if let Some(e) = self.entities.get_mut(&id) {
            for (slot, new) in e.weapons.iter_mut().zip(updated) {
                *slot = new;
            }
        }
        for (w, target_id) in shots {
            let (Some(from), Some(target)) =
                (self.entities.get(&id), self.entities.get(&target_id))
            else {
                continue;
            };
            let pid = self.next_projectile_id;
            if self.projectiles.contains_key(&pid) {
                continue;
            }
            let launched = shot(world, (from, id), w, target, target_id, pid);
            self.next_projectile_id = ProjectileId(pid.0.saturating_add(1));
            self.projectiles.insert(pid, launched);
        }
    }
}
