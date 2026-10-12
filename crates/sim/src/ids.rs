/// The integer simulation tick (SIM-002).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tick(pub u64);

/// An army slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Army(pub u8);

/// An entity identifier; spawned ids ascend from 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct EntityId(pub u32);

/// The fixed-point type of resources, health, progress, and timers.
pub type Q = fixed::types::I32F32;

/// A position in map units; `y` is height. Geometry is `f64` under ADR-0006's
/// exact operations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pos {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
