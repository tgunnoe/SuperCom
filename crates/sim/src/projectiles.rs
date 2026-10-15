//! The projectiles phase (SIM-077): lifetime, steering, gravity, the swept
//! impact test, flight, and the ground and water rules.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::combat::{GRAVITY, IMPACT_RADIUS, Impact, Projectile, ProjectileId, Scratch};
use crate::ids::{EntityId, Pos};
use crate::movement::{TICK_SECONDS, cell_of};
use crate::state::{Entity, State};
use crate::world::{ProjectileKind, World};

/// What one tick does to a projectile.
enum Outcome {
    Flying,
    /// Removed without an impact.
    Gone,
    Impact(Impact),
}

fn length(vec: [f64; 3]) -> f64 {
    libm::sqrt(vec.iter().map(|c| c * c).sum())
}

fn scaled(vec: [f64; 3], factor: f64) -> [f64; 3] {
    vec.map(|c| c * factor)
}

fn between(from: &Pos, to: &Pos) -> [f64; 3] {
    [to.x - from.x, to.y - from.y, to.z - from.z]
}

/// `vel` turned toward `want` by at most `max_turn` radians, keeping its
/// speed; at once when there is no limit (SIM-077).
fn turned(vel: [f64; 3], want: [f64; 3], max_turn: Option<f64>) -> [f64; 3] {
    let (speed, wanted) = (length(vel), length(want));
    if speed == 0.0 || wanted == 0.0 {
        return vel;
    }
    let (v, w) = (scaled(vel, 1.0 / speed), scaled(want, 1.0 / wanted));
    let [vx, vy, vz] = v;
    let [wx, wy, wz] = w;
    let angle = libm::acos((vx * wx + vy * wy + vz * wz).clamp(-1.0, 1.0));
    let sine = libm::sin(angle);
    let limit = max_turn.filter(|limit| angle > *limit && sine > 1.0e-12);
    match limit {
        None => scaled(w, speed),
        Some(limit) => {
            let keep = libm::sin(angle - limit) / sine;
            let lean = libm::sin(limit) / sine;
            scaled(
                [
                    vx * keep + wx * lean,
                    vy * keep + wy * lean,
                    vz * keep + wz * lean,
                ],
                speed,
            )
        }
    }
}

/// The velocity after steering, gravity, and acceleration (SIM-077).
fn steered(p: &Projectile, kind: &ProjectileKind, goal: Option<&Pos>) -> [f64; 3] {
    let mut vel = p.vel;
    if let Some(goal) = goal.filter(|_| kind.track_target) {
        let max_turn = kind
            .turn_rate
            .map(|rate| rate * (core::f64::consts::PI / 180.0) * TICK_SECONDS);
        vel = turned(vel, between(&p.pos, goal), max_turn);
    }
    if kind.use_gravity {
        vel[1] += GRAVITY * TICK_SECONDS;
    }
    if let Some(acceleration) = kind.acceleration {
        let speed = length(vel);
        let faster = speed + acceleration * TICK_SECONDS;
        // A speed already past the maximum is never cut back to it.
        let next = match kind.max_speed {
            Some(max) if speed >= max => speed,
            Some(max) => faster.min(max),
            None => faster,
        };
        if speed > 0.0 {
            vel = scaled(vel, next / speed);
        }
    }
    vel
}

#[expect(
    clippy::manual_midpoint,
    reason = "SIM-077 states the step as (v + v') / 2 in plain binary64 operations, not the library midpoint"
)]
fn advance(world: &World, entities: &BTreeMap<EntityId, Entity>, p: &mut Projectile) -> Outcome {
    if let Some(left) = p.lifetime {
        let left = left - TICK_SECONDS;
        if left <= 0.0 {
            return Outcome::Gone;
        }
        p.lifetime = Some(left);
    }
    let target = p
        .target
        .and_then(|id| entities.get(&id).map(|t| (id, t.pos)));
    let impact = |pos: Pos, on: Option<EntityId>| {
        Outcome::Impact(Impact {
            pos,
            on,
            damage: p.damage,
            damage_radius: p.damage_radius,
            damage_type: p.damage_type.clone(),
            army: p.army,
        })
    };
    let Some(kind_id) = &p.kind else {
        return match target {
            Some((id, pos)) => impact(pos, Some(id)),
            None => impact(p.aim, None),
        };
    };
    let kind = world.projectiles.get(kind_id);
    let old_vel = p.vel;
    if let Some(kind) = kind {
        p.vel = steered(p, kind, target.as_ref().map(|(_, pos)| pos));
    }
    let reach = length(p.vel) * TICK_SECONDS + IMPACT_RADIUS;
    match target {
        Some((id, pos)) if length(between(&p.pos, &pos)) <= reach => return impact(pos, Some(id)),
        None if length(between(&p.pos, &p.aim)) <= reach => return impact(p.aim, None),
        _ => {}
    }
    // The mean of the old and new velocity is exact for a constant
    // acceleration; the new velocity alone sinks below a parabola in
    // proportion to the flight time.
    p.pos.x += (old_vel[0] + p.vel[0]) / 2.0 * TICK_SECONDS;
    p.pos.y += (old_vel[1] + p.vel[1]) / 2.0 * TICK_SECONDS;
    p.pos.z += (old_vel[2] + p.vel[2]) / 2.0 * TICK_SECONDS;
    let map = &world.map;
    // Leaving the map is leaving the grid, not only an absent cell.
    let Some((cx, cz)) = cell_of(&p.pos).filter(|(x, z)| *x < map.width && *z < map.height) else {
        return Outcome::Gone;
    };
    let drowned = kind.is_some_and(|k| k.destroy_on_water)
        && map.water.is_some_and(|s| p.pos.y < s.to_num::<f64>());
    if drowned {
        return Outcome::Gone;
    }
    if p.pos.y < map.height_at(cx, cz).to_num::<f64>() {
        return impact(p.pos, None);
    }
    Outcome::Flying
}

impl State {
    /// The projectiles phase: each projectile, in ascending id, flies one
    /// tick; an impact is handed to the damage phase (SIM-077).
    pub(crate) fn run_projectiles(&mut self, scratch: &mut Scratch) {
        let world = Arc::clone(self.world());
        let ids: Vec<ProjectileId> = self.projectiles.keys().copied().collect();
        for id in ids {
            let Some(mut p) = self.projectiles.remove(&id) else {
                continue;
            };
            match advance(&world, &self.entities, &mut p) {
                Outcome::Flying => {
                    self.projectiles.insert(id, p);
                }
                Outcome::Gone => {}
                Outcome::Impact(impact) => scratch.impacts.push(impact),
            }
        }
    }
}
