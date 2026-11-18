use super::*;
use std::sync::Arc;
use supercom_sim::{Army, Layer, Q, World};

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12
}

fn pos(x: f64, y: f64, z: f64) -> Pos {
    Pos { x, y, z }
}

/// A position blends componentwise; a heading takes the shorter arc, across
/// the wrap in either direction.
#[test]
fn supercom_interpolation_cases() {
    assert_eq!(
        blend_position(pos(0.0, 0.0, 0.0), pos(10.0, 20.0, -30.0), 0.25),
        pos(2.5, 5.0, -7.5)
    );
    assert_eq!(
        blend_position(pos(2.0, 2.0, 2.0), pos(6.0, 4.0, -2.0), 0.0),
        pos(2.0, 2.0, 2.0)
    );

    let across = blend_heading(3.0, -2.5, 0.5);
    assert!(close(across, 3.391_592_653_589_793), "{across}");
    let back = blend_heading(-3.0, 2.5, 0.5);
    assert!(close(back, -3.391_592_653_589_793), "{back}");
    let plain = blend_heading(0.2, 0.6, 0.5);
    assert!(close(plain, 0.4), "{plain}");
    assert!(close(blend_heading(3.0, -2.5, 0.0), 3.0));
    let whole = blend_heading(3.0, -2.5, 1.0);
    assert!(close(whole, 3.783_185_307_179_586), "{whole}");
}

/// The kept values are the state's, by id.
#[test]
fn supercom_interpolation_keeps_the_values_by_id() {
    let mut world = World::empty();
    world.units.clear();
    let mut state = State::with_world(Arc::new(world), 0);
    let first = state.spawn(Army(1), "a", pos(1.0, 2.0, 3.0), Q::from_num(1));
    let second = state.spawn(Army(2), "b", pos(4.0, 5.0, 6.0), Q::from_num(1));
    if let Some(entity) = state.entities.get_mut(&second) {
        entity.heading = 1.5;
        entity.layer = Layer::Air;
    }
    let kept = Kept::of(&state);
    assert_eq!(kept.entity(first), Some((pos(1.0, 2.0, 3.0), 0.0)));
    assert_eq!(kept.entity(second), Some((pos(4.0, 5.0, 6.0), 1.5)));
    assert_eq!(kept.entity(EntityId(99)), None);
    assert_eq!(kept.projectile(ProjectileId(1)), None);
}
