//! Evidence for `docs/specs/determinism.md`.
//!
//! This crate is not a simulation and not a network layer. It holds the small
//! pieces the float clauses are measured against: the `libm` and fixed-point
//! operations the float strategy compares, and the canonical NaN rule. Its golden-vector tests run on every platform the workspace builds
//! for; the committed constants were produced on x86_64-linux, and no result
//! from another architecture is claimed (see `docs/adr/0006-float-strategy.md`).
//!
//! `cargo run -p supercom-detbench --example golden` prints the constants the
//! tests compare against; `cargo run --release -p supercom-detbench --example
//! bench` prints the throughput figures the ADR cites.

pub mod fixedpoint;
pub mod floats;
