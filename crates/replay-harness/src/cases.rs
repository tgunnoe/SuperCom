//! The two cases of DET-024: a NaN of each origin in a position, and the
//! recursion depth of the plugin host.
//!
//! Both drive the plugin host from outside the sim: no plugin runs inside
//! the tick loop in this version.

use std::sync::Arc;
use std::thread;

use supercom_plugin::{Error as PluginError, Host};
use supercom_sim::{Army, Command, Faction, Order, Pos, Q, QueueMode, Tick, World};

use crate::{HarnessError, hex};

/// A component implementing the whole `plugin` world, with the bodies of
/// `step` and `nan-bits` left to the case (the plugin host's own unit tests
/// assemble theirs from the same text).
const WORLD: &str = r#"
    (component
      (import "supercom:engine/host@0.1.0"
        (instance (export "log" (func (param "message" string)))))
      (core module $m
        (memory (export "memory") 1)
        (data (i32.const 8) "hi")
        (func (export "describe") (result i32)
          (i32.store (i32.const 0) (i32.const 8))
          (i32.store (i32.const 4) (i32.const 2))
          (i32.const 0))
        (func $step (export "step") (param i64) (result i64) {step})
        (func (export "nan-bits") (result i64) {nan_bits})
        (func (export "spin") (loop (br 0))))
      (core instance $i (instantiate $m))
      (func (export "describe") (result string)
        (canon lift (core func $i "describe") (memory $i "memory")))
      (func (export "step") (param "tick" u64) (result u64)
        (canon lift (core func $i "step")))
      (func (export "nan-bits") (result u64)
        (canon lift (core func $i "nan-bits")))
      (func (export "spin")
        (canon lift (core func $i "spin"))))
"#;

/// A guest runs on the caller's stack, so the cases run on a thread whose
/// stack holds the host's whole budget with room to spare.
const THREAD_STACK_BYTES: usize = 8 * 1024 * 1024;

/// Fuel for a call into a case's component: more than any probe uses.
const FUEL: u64 = u64::MAX;

/// The tick at which the extra entity is spawned, and the ticks stepped.
const NAN_AT: u64 = 50;
const NAN_TICKS: u64 = 10;

/// What the payload-NaN case reports (DET-024).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NanCase {
    /// The compared lines: one `case nan tick <t> <hex>` per stepped tick.
    pub lines: Vec<String>,
    /// The bits of the NaN the host's arithmetic produced, for stderr.
    pub host_nan_bits: u64,
    /// The bits of the NaN the plugin returned, for stderr.
    pub plugin_nan_bits: u64,
    /// The bits of the extra entity's final position, for stderr.
    pub final_position_bits: [u64; 3],
    /// Whether a component of that position is still a NaN.
    pub position_is_nan: bool,
}

/// What the recursion case reports (DET-024).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecursionCase {
    /// The compared line: the kind of trap an unbounded recursion raises.
    pub line: String,
    /// The largest depth that returned on this host, for stderr.
    pub depth: u64,
}

fn component(step: &str, nan_bits: &str) -> Result<Vec<u8>, HarnessError> {
    let text = WORLD
        .replace("{step}", step)
        .replace("{nan_bits}", nan_bits);
    wat::parse_str(&text).map_err(|err| HarnessError::Case(format!("the component text: {err}")))
}

/// Run `body` on a thread with [`THREAD_STACK_BYTES`] of stack.
fn on_big_stack<T: Send>(
    body: impl FnOnce() -> Result<T, HarnessError> + Send,
) -> Result<T, HarnessError> {
    thread::scope(|scope| {
        thread::Builder::new()
            .stack_size(THREAD_STACK_BYTES)
            .spawn_scoped(scope, body)
            .map_err(|err| HarnessError::Case(format!("spawning the case thread: {err}")))?
            .join()
            .map_err(|_| HarnessError::Case("the case thread panicked".to_owned()))?
    })
}

/// Zero over zero, computed at run time: the barrier stops the compiler
/// folding it into a constant of its own choosing.
#[allow(
    clippy::eq_op,
    reason = "the case needs the NaN the host's own division of zero by zero produces"
)]
fn host_nan() -> f64 {
    let zero = std::hint::black_box(0.0_f64);
    zero / zero
}

/// The payload-NaN case on `world`: a NaN the host's arithmetic computes
/// and a NaN a plugin returns, placed in one entity's position, stepped
/// through the sim. Only DET-022's canonicalization makes the digests agree
/// across hosts.
///
/// # Errors
///
/// [`HarnessError`] from the sim or the plugin host.
pub fn nan_case(world: &Arc<World>) -> Result<NanCase, HarnessError> {
    let world = Arc::clone(world);
    on_big_stack(move || {
        let mut state = world.skirmish(1, &[Faction::UEF, Faction::Cybran])?;
        for _ in 0..NAN_AT {
            state.step(&[])?;
        }
        let uef = state
            .entities
            .values()
            .find(|e| e.army == Army(1))
            .ok_or_else(|| HarnessError::Case("no UEF commander".to_owned()))?;
        let (home, blueprint) = (uef.pos, uef.blueprint.clone());

        let host_nan = host_nan();
        let mut plugin = Host::new()?.load(&component(
            "(i64.const 0)",
            "(i64.reinterpret_f64 (f64.div (f64.const 0) (f64.const 0)))",
        )?)?;
        let plugin_nan = f64::from_bits(plugin.nan_bits(FUEL)?);

        let extra = state.spawn(
            Army(1),
            &blueprint,
            Pos {
                x: host_nan,
                y: plugin_nan,
                z: home.z,
            },
            Q::ONE,
        );
        let mover = Command {
            tick: Tick(NAN_AT),
            source: 0,
            seq: 0,
            army: Army(1),
            units: vec![extra],
            order: Order::Move { to: home },
            mode: QueueMode::Replace,
        };
        let mut lines = Vec::new();
        for step in 0..NAN_TICKS {
            let batch: &[Command] = if step == 0 {
                std::slice::from_ref(&mover)
            } else {
                &[]
            };
            let digest = state.step(batch)?;
            lines.push(format!("case nan tick {} {}", NAN_AT + step, hex(&digest)));
        }
        let end = state
            .entities
            .get(&extra)
            .map(|e| e.pos)
            .ok_or_else(|| HarnessError::Case("the extra entity vanished".to_owned()))?;
        Ok(NanCase {
            lines,
            host_nan_bits: host_nan.to_bits(),
            plugin_nan_bits: plugin_nan.to_bits(),
            final_position_bits: [end.x, end.y, end.z].map(f64::to_bits),
            position_is_nan: end.x.is_nan() || end.y.is_nan() || end.z.is_nan(),
        })
    })
}

/// Whether a recursion to `depth` returns on this host: `Ok(true)` if it
/// does, `Ok(false)` if it exhausts the stack.
fn returns(host: &Host, bytes: &[u8], depth: u64) -> Result<bool, HarnessError> {
    match host.load(bytes)?.step(depth, FUEL) {
        Ok(_) => Ok(true),
        Err(PluginError::Trap { reason }) if reason.contains("call stack exhausted") => Ok(false),
        Err(other) => Err(HarnessError::Case(format!(
            "a recursion to depth {depth} ended with {other}, not a result or stack exhaustion"
        ))),
    }
}

/// The recursion case: the largest depth that returns, found by bisection
/// with a fresh instance per probe (recorded, not compared: DET-023), and
/// the kind of trap an unbounded recursion raises (compared).
///
/// # Errors
///
/// [`HarnessError`] when the host ends a probe any other way than a result
/// or stack exhaustion, or when the unbounded recursion does not trap with
/// stack exhaustion.
pub fn recursion_case() -> Result<RecursionCase, HarnessError> {
    on_big_stack(|| {
        let host = Host::new()?;
        let bounded = component(
            "(if (result i64) (i64.eqz (local.get 0)) (then (i64.const 0)) \
             (else (call $step (i64.sub (local.get 0) (i64.const 1)))))",
            "(i64.const 0)",
        )?;
        let mut low = 0_u64;
        let mut high = 1_u64;
        while returns(&host, &bounded, high)? {
            low = high;
            high = high.checked_mul(2).ok_or_else(|| {
                HarnessError::Case("no recursion depth exhausted the stack".to_owned())
            })?;
        }
        while high - low > 1 {
            let middle = low + (high - low) / 2;
            if returns(&host, &bounded, middle)? {
                low = middle;
            } else {
                high = middle;
            }
        }

        let unbounded = component("(call $step (local.get 0))", "(i64.const 0)")?;
        match host.load(&unbounded)?.step(0, FUEL) {
            Err(PluginError::Trap { reason }) if reason.contains("call stack exhausted") => {
                Ok(RecursionCase {
                    line: "case recursion unbounded trap call-stack-exhausted".to_owned(),
                    depth: low,
                })
            }
            other => Err(HarnessError::Case(format!(
                "an unbounded recursion ended with {other:?}, not stack exhaustion"
            ))),
        }
    })
}
