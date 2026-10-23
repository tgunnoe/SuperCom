//! Guards on the build phase (docs/specs/sim.md, SIM-041, SIM-059 to
//! SIM-061): assist range, Guard targets, completion, queue progress, and
//! each site check, one proposition per test. All synthetic.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

mod support;

use std::sync::Arc;

use supercom_sim::{
    Army, BuildRestriction, Entity, EntityId, MapGrid, Marker, MarkerKind, Order, QueueMode, State,
    World,
};
use support::{
    build_at, builder, flat_map, frac, kind, order_cmd, pos, q, restricted, state, step, structure,
    world,
};

fn entity(s: &State, id: EntityId) -> &Entity {
    s.entities.get(&id).expect("the entity exists")
}

fn rich(w: &Arc<World>) -> State {
    state(w, q(1_000_000), q(1_000_000))
}

/// A worker of rate 10 and a helper of rate 5 (which builds nothing)
/// around `gen`: build time `gen_time`, so per tick the worker adds 1 and
/// the helper 1/2 of work.
fn gen_world(gen_time: i64, map: MapGrid) -> Arc<World> {
    world(
        vec![
            builder("worker", 10, &["gen", "mex"]),
            builder("helper", 5, &[]),
            structure("gen", 60, 600, gen_time),
            restricted(
                structure("mex", 60, 600, gen_time),
                BuildRestriction::OnMassDeposit,
            ),
        ],
        Vec::new(),
        map,
    )
}

fn guard(target: EntityId) -> Order {
    Order::Guard { target }
}

/// A worker at (4.5, 4.5) commanded to build a site at (4.5, 8.5), and a
/// helper at `helper_at` guarding the worker; both ids are fixed: worker 1,
/// helper 2, site 3.
fn assist_setup(helper_at: (f64, f64)) -> State {
    let w = gen_world(600, flat_map(64));
    let mut s = rich(&w);
    let b = s.spawn(Army(1), "worker", pos(4.5, 0.0, 4.5), q(100));
    let h = s.spawn(
        Army(1),
        "helper",
        pos(helper_at.0, 0.0, helper_at.1),
        q(100),
    );
    step(
        &mut s,
        &[
            order_cmd(0, 0, b, build_at("gen", 4.5, 8.5), QueueMode::Replace),
            order_cmd(0, 1, h, guard(b), QueueMode::Replace),
        ],
    );
    s
}

/// An assister out of range of the site takes no work item.
#[test]
fn an_assister_out_of_range_takes_no_work() {
    let mut s = assist_setup((50.0, 50.0));
    step(&mut s, &[]);
    assert_eq!(entity(&s, EntityId(2)).work, None);
    // The worker alone: 60 mass over 60 seconds is 1 per second, and a
    // tenth of it per tick is raw (1 << 32) / 10 = 429496729.
    let ledger = s.armies.get(&Army(1)).expect("army 1 exists").ledger;
    assert_eq!(ledger.requested_mass.to_bits(), 429_496_729);
}

/// An assister that was in range when assigned and has moved out before
/// the next build phase adds nothing.
#[test]
fn an_assister_that_moved_out_adds_nothing() {
    let mut s = assist_setup((5.5, 4.5));
    assert_eq!(entity(&s, EntityId(2)).work, Some(EntityId(3)), "assigned");
    s.entities.get_mut(&EntityId(2)).expect("the helper").pos = pos(50.0, 0.0, 50.0);
    step(&mut s, &[]);
    // The worker's 1 only; with the helper it would be 3/2.
    assert_eq!(entity(&s, EntityId(3)).built, q(1));
}

/// A Guard naming the unit under construction itself assists it.
#[test]
fn a_guard_on_the_site_itself_assists() {
    let mut s = assist_setup((50.0, 50.0));
    let helper = EntityId(2);
    s.entities.get_mut(&helper).expect("the helper").pos = pos(5.5, 0.0, 4.5);
    step(
        &mut s,
        &[order_cmd(
            1,
            2,
            helper,
            guard(EntityId(3)),
            QueueMode::Replace,
        )],
    );
    assert_eq!(entity(&s, helper).work, Some(EntityId(3)));
    step(&mut s, &[]);
    // Tick 1 is the worker's 1; tick 2 adds the worker's 1 and the
    // helper's 1/2.
    assert_eq!(entity(&s, EntityId(3)).built, frac(5, 2));
}

/// Completion pops the builder's build order and clears the assister's
/// work but leaves the assister's Guard in place.
#[test]
fn completion_leaves_an_assisters_guard() {
    let w = gen_world(6, flat_map(64));
    let mut s = rich(&w);
    let b = s.spawn(Army(1), "worker", pos(4.5, 0.0, 4.5), q(100));
    let h = s.spawn(Army(1), "helper", pos(5.5, 0.0, 4.5), q(100));
    step(
        &mut s,
        &[
            order_cmd(0, 0, b, build_at("gen", 4.5, 8.5), QueueMode::Replace),
            order_cmd(0, 1, h, guard(b), QueueMode::Replace),
        ],
    );
    for _ in 0..6 {
        step(&mut s, &[]);
    }
    assert_eq!(s.counters.builds_completed, 1);
    assert!(entity(&s, EntityId(3)).complete);
    assert!(entity(&s, b).orders.is_empty(), "the build order is popped");
    assert_eq!(entity(&s, h).orders.front(), Some(&guard(b)));
    assert_eq!(entity(&s, h).work, None);
}

/// After a refused head, the next queued build starts in the same phase.
#[test]
fn a_refused_head_does_not_stop_the_queue() {
    let w = gen_world(600, flat_map(64));
    let mut s = rich(&w);
    let b = s.spawn(Army(1), "worker", pos(4.5, 0.0, 4.5), q(100));
    step(
        &mut s,
        &[
            order_cmd(0, 0, b, build_at("nothing", 4.5, 8.5), QueueMode::Replace),
            order_cmd(0, 1, b, build_at("gen", 4.5, 8.5), QueueMode::Append),
        ],
    );
    assert_eq!(s.counters.commands_ignored, 1);
    assert_eq!(entity(&s, b).work, Some(EntityId(2)), "started at tick 0");
}

/// One refused build at (4.5, 8.5) for a builder at `builder_at`, with
/// `before` run on the state first; true when it was dropped.
fn refused(builder_at: (f64, f64), before: impl FnOnce(&mut State)) -> bool {
    let w = gen_world(600, flat_map(64));
    let mut s = rich(&w);
    let b = s.spawn(
        Army(1),
        "worker",
        pos(builder_at.0, 0.0, builder_at.1),
        q(100),
    );
    before(&mut s);
    let others = s.entities.len();
    step(
        &mut s,
        &[order_cmd(
            0,
            0,
            b,
            build_at("gen", 4.5, 8.5),
            QueueMode::Replace,
        )],
    );
    let dropped = s.counters.commands_ignored == 1;
    assert_eq!(
        s.entities.len(),
        if dropped { others } else { others + 1 },
        "a refusal spawns nothing and a start spawns one site"
    );
    dropped
}

/// A site overlapping an unfinished entity's footprint is refused.
#[test]
fn a_site_over_an_unfinished_entity_is_refused() {
    assert!(refused((4.5, 4.5), |s| {
        s.spawn_under_construction(Army(1), "gen", pos(4.5, 0.0, 8.5))
            .expect("gen has a build time");
    }));
    // Control: the same site elsewhere is free.
    assert!(!refused((4.5, 4.5), |s| {
        s.spawn_under_construction(Army(1), "gen", pos(20.5, 0.0, 20.5))
            .expect("gen has a build time");
    }));
}

/// A site covering a mobile unit's cell is refused, the builder's own
/// included.
#[test]
fn a_site_over_a_mobile_unit_is_refused() {
    assert!(refused((4.5, 8.5), |_| {}), "the builder's own cell");
    assert!(
        refused((4.5, 4.5), |s| {
            s.spawn(Army(1), "helper", pos(4.5, 0.0, 8.5), q(100));
        }),
        "another unit's cell"
    );
}

/// A mass extractor placed with a marker at an offset of exactly 1 on
/// `z`, or at a height differing by exactly 1, is refused; one on the
/// marker is placed.
#[test]
fn a_marker_one_away_on_z_or_y_does_not_hold_the_site() {
    let placed = |marker_at: (f64, f64, f64)| {
        let mut w = World::empty();
        w.units = [
            builder("worker", 10, &["mex"]),
            restricted(
                structure("mex", 60, 600, 600),
                BuildRestriction::OnMassDeposit,
            ),
        ]
        .into_iter()
        .map(|k| (k.id.clone(), k))
        .collect();
        w.map = flat_map(64);
        w.markers = vec![Marker {
            name: "Mass 1".to_owned(),
            kind: MarkerKind::Mass,
            pos: pos(marker_at.0, marker_at.1, marker_at.2),
        }];
        let mut s = rich(&Arc::new(w));
        let b = s.spawn(Army(1), "worker", pos(4.5, 0.0, 4.5), q(100));
        step(
            &mut s,
            &[order_cmd(
                0,
                0,
                b,
                build_at("mex", 4.5, 8.5),
                QueueMode::Replace,
            )],
        );
        s.entities.len() == 2
    };
    // The site's height is the flat map's 0.
    assert!(placed((4.5, 0.0, 8.5)), "a marker on the site holds it");
    assert!(!placed((4.5, 0.0, 9.5)), "one away on z");
    assert!(!placed((4.5, 1.0, 8.5)), "one away in height");
}

/// A builder exactly 5 from the footprint's nearest edge starts; just
/// beyond waits.
#[test]
fn a_builder_exactly_at_the_build_range_starts() {
    let start = |x: f64| {
        let w = gen_world(600, flat_map(64));
        let mut s = rich(&w);
        let b = s.spawn(Army(1), "worker", pos(x, 0.0, 8.5), q(100));
        step(
            &mut s,
            &[order_cmd(
                0,
                0,
                b,
                build_at("gen", 8.5, 8.5),
                QueueMode::Replace,
            )],
        );
        entity(&s, b).work.is_some()
    };
    // The site covers x in 8..9; 14.0 is exactly 5 from its edge.
    assert!(start(14.0));
    assert!(!start(14.25));
}

/// On a map with relief the structure under construction stands at the
/// height of the cell under the site, not at the order's y.
#[test]
fn the_site_stands_at_the_terrain_height() {
    // Sample (x, z) rises 16 raw units per column; at scale 1/128 that is
    // 1/8 per column, so column 8 stands at exactly 1.
    let size = 16_u32;
    let samples: Vec<i16> = (0..=size)
        .flat_map(|_| (0..=size).map(|x| i16::try_from(x * 16).expect("fits")))
        .collect();
    let map = MapGrid::from_heightmap(size, size, &samples, 0.007_812_5).expect("a ramp");
    let w = gen_world(600, map);
    let mut s = rich(&w);
    let b = s.spawn(Army(1), "worker", pos(8.5, 0.0, 4.5), q(100));
    step(
        &mut s,
        &[order_cmd(
            0,
            0,
            b,
            build_at("gen", 8.5, 8.5),
            QueueMode::Replace,
        )],
    );
    assert_eq!(entity(&s, EntityId(2)).pos.y, 1.0);
}

/// A Stop at the head of a non-builder's queue stays there.
#[test]
fn a_non_builders_stop_stays_queued() {
    let w = world(vec![kind("rock", &["MOBILE"])], Vec::new(), flat_map(8));
    let mut s = rich(&w);
    let r = s.spawn(Army(1), "rock", pos(1.5, 0.0, 1.5), q(100));
    step(
        &mut s,
        &[order_cmd(0, 0, r, Order::Stop, QueueMode::Replace)],
    );
    assert_eq!(entity(&s, r).orders.front(), Some(&Order::Stop));
}
