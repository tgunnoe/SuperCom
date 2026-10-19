//! The skirmish the committed log records (DET-029).
//!
//! The UEF commander walks to the mass marker beside its start, builds an
//! extractor and three point defences there until the economy stalls, then
//! walks to a flat run of land beside the Cybran start, where the Cybran
//! commander has been waiting since the first tick. Both are told to attack
//! once the UEF commander has arrived; the Cybran commander dies first, which
//! defeats its army and declares the UEF one the winner. Flat ground keeps
//! the shells from missing on relief.

use std::sync::Arc;
use supercom_sim::{
    Army, Command, EntityId, Layer, MarkerKind, Order, Pos, QueueMode, REPLAY_FORMAT_VERSION,
    Replay, ReplayHeader, Tick, World,
};

use crate::HarnessError;

/// The ticks the log runs; its last checkpoint is the tick before.
pub const LOG_TICKS: u64 = 4_600;

/// A checkpoint every this many ticks, and at the last.
const CHECKPOINT_EVERY: u64 = 100;

/// The seed in the header.
const SEED: u64 = 0x5343_4d50_3031_3200;

/// When the UEF commander leaves its base, and when both attack.
const LEAVE_AT: u64 = 1_400;
const ATTACK_AT: u64 = 3_400;

/// Whether the log carries a checkpoint at `tick`.
#[must_use]
pub fn is_checkpoint(tick: u64) -> bool {
    tick % CHECKPOINT_EVERY == CHECKPOINT_EVERY - 1 || tick + 1 == LOG_TICKS
}

fn missing(what: &str) -> HarnessError {
    HarnessError::Case(format!("the map has no {what}"))
}

/// The row and start column of a run of nine land cells at one height,
/// nearest the Cybran start marker by Manhattan distance, then row, then
/// column.
fn flat_run(world: &World) -> Result<(u32, u32, f64), HarnessError> {
    let marker = world
        .markers
        .iter()
        .find(|m| m.kind == MarkerKind::Army(2))
        .ok_or_else(|| missing("Cybran start marker"))?
        .pos;
    let map = &world.map;
    let mut best: Option<((u64, u32, u32), f64)> = None;
    for z in 0..map.height {
        for x in 0..map.width.saturating_sub(9) {
            let h = map.height_at(x, z);
            let flat =
                (x..x + 9).all(|c| map.height_at(c, z) == h && map.passable(Layer::Land, c, z));
            if !flat {
                continue;
            }
            let near =
                (f64::from(x) + 4.5 - marker.x).abs() + (f64::from(z) + 0.5 - marker.z).abs();
            // Rounded to whole units, so equal keys fall to row then column.
            let key = (near.round().to_bits(), z, x);
            if best.as_ref().is_none_or(|(k, _)| key < *k) {
                best = Some((key, h.to_num::<f64>()));
            }
        }
    }
    let ((_, z, x), y) = best.ok_or_else(|| missing("flat run of land near the Cybran start"))?;
    Ok((x, z, y))
}

/// Commands in schedule order, each source's sequence counting from 0.
struct Issuer {
    commands: Vec<Command>,
    seq: [u32; 2],
}

impl Issuer {
    fn issue(&mut self, tick: u64, source: u8, unit: EntityId, order: Order, mode: QueueMode) {
        let slot = usize::from(source);
        self.commands.push(Command {
            tick: Tick(tick),
            source,
            seq: self.seq[slot],
            army: Army(source + 1),
            units: vec![unit],
            order,
            mode,
        });
        self.seq[slot] += 1;
    }
}

/// The spot `dz` units along z from the commander's stand beside the mass
/// marker.
fn beside(mass: Pos, dz: f64) -> Pos {
    Pos {
        x: mass.x + 1.0,
        y: mass.y,
        z: mass.z + dz,
    }
}

/// The UEF commander's first phase: stand by the marker, build the
/// extractor on it, then three point defences, each from a stand two units
/// from its site (a build reaches five). The defences' energy cost outruns
/// the stock, which stalls the economy.
fn build_at_base(at: &mut Issuer, uef: EntityId, mass: Pos) {
    let to = |dz: f64| Order::Move {
        to: beside(mass, dz),
    };
    at.issue(0, 0, uef, to(0.0), QueueMode::Replace);
    at.issue(
        0,
        0,
        uef,
        Order::BuildMobile {
            blueprint: "ueb1103".to_owned(),
            at: mass,
        },
        QueueMode::Append,
    );
    for dz in [-8.0, -16.0, -24.0] {
        at.issue(0, 0, uef, to(dz + 2.0), QueueMode::Append);
        at.issue(
            0,
            0,
            uef,
            Order::BuildMobile {
                blueprint: "ueb2101".to_owned(),
                at: beside(mass, dz),
            },
            QueueMode::Append,
        );
    }
}

/// The header of a log played on `world`: its digests, no plugins, and one
/// player per faction named by the string (DET-030).
fn header(world: &World) -> ReplayHeader {
    ReplayHeader {
        format_version: REPLAY_FORMAT_VERSION,
        engine_version: "supercom-sim 0.0.1".to_owned(),
        map: world.map_name.clone(),
        map_digest: world.map_digest,
        catalog_digest: world.catalog_digest,
        plugins: Vec::new(),
        players: vec![(Army(1), "UEF".to_owned()), (Army(2), "Cybran".to_owned())],
        seed: SEED,
    }
}

/// The log without checkpoints: the header of `world` and the commands.
pub fn replay(world: &Arc<World>) -> Result<Replay, HarnessError> {
    let probe = Arc::clone(world).replay_start(&header(world))?;
    let commander = |army: Army| {
        probe
            .entities
            .values()
            .find(|e| e.army == army)
            .map(|e| e.id)
            .ok_or_else(|| missing("commander"))
    };
    let (uef, cybran) = (commander(Army(1))?, commander(Army(2))?);

    let start = world
        .markers
        .iter()
        .find(|m| m.kind == MarkerKind::Army(1))
        .ok_or_else(|| missing("UEF start marker"))?
        .pos;
    let mass = world
        .markers
        .iter()
        .filter(|m| m.kind == MarkerKind::Mass)
        .min_by(|a, b| {
            supercom_sim::distance_xz(&a.pos, &start)
                .total_cmp(&supercom_sim::distance_xz(&b.pos, &start))
        })
        .ok_or_else(|| missing("mass marker"))?
        .pos;
    let (x, z, y) = flat_run(world)?;
    let flat = |dx: f64| Pos {
        x: f64::from(x) + dx,
        y,
        z: f64::from(z) + 0.5,
    };
    let mut at = Issuer {
        commands: Vec::new(),
        seq: [0; 2],
    };
    build_at_base(&mut at, uef, mass);
    at.issue(
        0,
        1,
        cybran,
        Order::Move { to: flat(8.5) },
        QueueMode::Replace,
    );
    at.issue(
        LEAVE_AT,
        0,
        uef,
        Order::Move { to: flat(0.5) },
        QueueMode::Replace,
    );
    at.issue(
        ATTACK_AT,
        0,
        uef,
        Order::Attack { target: cybran },
        QueueMode::Replace,
    );
    at.issue(
        ATTACK_AT,
        1,
        cybran,
        Order::Attack { target: uef },
        QueueMode::Replace,
    );

    let mut commands = at.commands;
    commands.sort_by_key(|c| (c.tick, c.source, c.seq));
    Ok(Replay {
        header: header(world),
        commands,
        checkpoints: Vec::new(),
    })
}
