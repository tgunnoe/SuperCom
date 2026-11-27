# ADR-0006: Float strategy for the deterministic sim

**Status:** ACCEPTED (ratified by the head on 2026-10-10; U1 stays open as
verification until the DET-024 replay comparison runs in CI on both
architectures and once on aarch64-darwin)

**Date:** 2026-10-09

---

## Context

The sim must be a pure function of its initial state and command stream, on
every supported platform (AGENTS.md R2, I2), for lockstep play and replays.
AGENTS.md U1 left open whether that means fixed-point arithmetic or IEEE
floats with deterministic transcendental functions, because transcendental
and fused-multiply-add behaviour is not obviously the same on x86_64 and
aarch64. `docs/specs/determinism.md` now states the contract; this ADR
chooses the arithmetic and records what has and has not been measured.

Four hazards decide the question:

- The standard library documents the precision of `sin`, `exp`, `powf`,
  `powi`, `atan2`, and the other transcendentals as non-deterministic: it
  varies by platform, Rust version, and within one execution (DET-014).
- A NaN produced by arithmetic has an unspecified sign and payload (DET-016).
  If one reaches the state hash, two correct builds disagree.
- A multiply followed by an add may or may not be fused. Rust documents
  `mul_add` as fused and exact (DET-015); whether the compiler also fuses a
  separate multiply and add is a property of the toolchain, which DET-018
  checks on x86_64 only.
- Plugins run under wasmtime with NaN canonicalization (ADR-0001), which
  canonicalizes the results of float arithmetic only; a NaN a guest builds by a
  sign operation, a reinterpretation, a constant, or a load keeps its bits.
  Those bits are the same on every host, and the sim canonicalizes every `f64`
  at the serialization boundary whatever its origin (DET-022), so a plugin's
  result and the sim's own arithmetic still hash alike.

## Decision

Ratified by the head on 2026-10-10 after the design review; nothing
here changes ADR-0001 through ADR-0005.

- The v0 sim computes in binary64 IEEE arithmetic with a restricted
  vocabulary: `+`, `-`, `*`, `/`, `sqrt`, and `mul_add`, which the standard
  library documents as exactly specified (DET-015). Other exactly specified
  operations, such as `floor` and `abs`, join the list when a clause cites
  their documentation.
- Every transcendental goes through the `libm` crate, a pure-Rust library
  (DET-019, DET-020). Its `arch` feature may be on or off, because it only swaps
  assembly in for exact operations and Cargo enables it in workspace builds; the
  vectors hold both ways. The sim does not call the
  standard library's sine, cosine, exponential, logarithm, power, or arctangent
  (DET-014). The sim crate should enforce this with `disallowed-methods` in its
  `clippy.toml`; that file is outside this change.
- A NaN never reaches hashed state or a replay unchanged. The sim either never
  produces one, by validating inputs, or maps every NaN to the canonical pattern
  `0x7ff8000000000000` at the serialization boundary (DET-022). The plugin
  host's canonicalization covers only a guest's arithmetic results, so this
  boundary canonicalization is what makes a plugin's result and the sim's own
  arithmetic hash as one pattern.
- The 64-bit fixed-point format with 32 fractional bits (`I32F32`) stays
  available for a quantity that must be exact without floating point, chosen
  per quantity in the sim specification (DET-021). It is also the fallback if
  the cross-architecture replay comparison (DET-024) finds a divergence the
  float path cannot close.
- Algebraic float operators, which license reassociation, and any build flag
  that relaxes IEEE semantics are forbidden in the sim crate.
- U1 stays open as verification: it closes when the replay comparison of
  DET-024 runs in CI on x86_64 and aarch64 and once on aarch64-darwin; it has
  passed once by hand on two Linux hosts. This ADR was ACCEPTED on 2026-10-10, and a
  DET-024 failure reopens it; the claims below are x86_64 claims.

## Evidence

All figures come from one machine: AMD Ryzen 9 9950X, x86_64-linux, rustc 1.99.0
(b940084d7 2026-09-28), GNU libc 2.44 as reported by `ldd --version` in the
devshell, cargo-nextest 0.9.146. Every command runs in the devshell from the
repository root (`nix-shell --run '<command>'`).

### Golden vectors

`cargo nextest run -p supercom-detbench` runs 6 tests, all passing: the
golden vectors and properties behind DET-002, DET-003, and DET-018 through
DET-022 (the schedule and tick-hash models behind DET-010, DET-011, and
DET-013 were retired once the sim's own tests existed), plus two sanity tests (the SHA-256
known-answer vector, and `libm` within four ulps of the platform's `sin` and
`exp`). The constants were printed by
`cargo run -p supercom-detbench --example golden`, which calls the code the
tests call, so a constant proves only that the code has not changed since; the
independent checks are the `f64::sqrt` equality, the hand-built hash layout,
the 128-bit integer model of the fixed-point sequence, and the ulp test.

The tests also pass in release mode and with `RUSTFLAGS='-C target-cpu=native'`,
which on this machine enables `fma`, `avx2`, and `sha`, in debug and release
(`cargo nextest run [--release] -p supercom-detbench`, with
`CARGO_TARGET_DIR` set apart for the native build). They pass as well in the
workspace configuration, `cargo nextest run --workspace -E
'package(supercom-detbench)'`: there `cargo tree --workspace -e features -i
libm` shows the `arch` feature of `libm` enabled, through cranelift-codegen in
the plugin host's dependencies, while `cargo tree -p supercom-detbench -e
features -i libm` shows it off. The vectors are therefore the same with `arch`
on and off. That run matters for
aarch64: FMA instructions are baseline there but not on plain x86_64, so a
compiler that fused a separate multiply and add would show only on a build that
has them.

To check that each test can fail, fourteen mutations were applied by hand, one
at a time, and each turned at least one test red: an altered
digit or bit in each of the sine, exponential, square-root, basic-operation,
fixed-point, and hash constants; a changed sort key; a command placed one tick late; a late-command
check shifted by one; a hash chain without the tick; a divergence report off by
one; a different canonical NaN; truncation instead of floor in the fixed-point
model; and commands applied in reverse within a tick.

### Platform libm against `libm`

`cargo run --release -p supercom-detbench --example bench` ran three times
with the same counts each time. On a million inputs (sine over [-100, 100),
exponential over [-20, 20)), the platform function and the `libm` crate
return different bits for:

| Function | Inputs differing | Worst difference |
| :------- | ---------------: | :--------------- |
| `sin`    |    32,572 (3.1%) | 1 ulp            |
| `exp`    |   102,783 (9.8%) | 1 ulp            |
| `sqrt`   |           0 (0%) | 0 ulp            |

Two peers that disagree on a few percent of calls would diverge as soon as an
affected result reaches state, so the standard library's transcendentals are
unusable for lockstep even where each is individually accurate. Both functions
here are the platform's own, glibc's in this devshell; another platform's
would differ again, and that is the point of DET-014. `sqrt` agrees on every input, as IEEE 754
requires.

### Throughput

The same example, release mode, best of seven timings, three runs within 2% of
each other:

| Measure                     | Platform | `libm` or fixed | Ratio |
| :-------------------------- | -------: | --------------: | ----: |
| `sin` (ns per call)         |     12.5 |            13.3 | 1.06x |
| `exp` (ns per call)         |      3.9 |             5.2 | 1.34x |
| `sqrt` (ns per call)        |      1.4 |             5.5 |  4.0x |
| multiply-accumulate (ns/op) |     0.54 |            0.56 | 1.03x |

Reading the table: `libm`'s sine costs about as much as the platform's, and
its exponential about a third more, which is small against the cost of
determinism. `libm::sqrt` is software with default features off and loses by
4x, which is why the sim calls `f64::sqrt`, an IEEE operation that agrees
with it bit for bit. The multiply-accumulate loop is latency-bound on the
accumulator, so it shows that `I32F32` is no slower than `f64` for a
dependent chain on this machine and nothing about vectorized throughput.

The state hash, SHA-256 over the whole state each tick, runs at about 2.5 GiB/s
here (the CPU has the SHA extensions): 24 us for a 64 KiB state, 0.39 ms for
1 MiB, 1.5 ms for 4 MiB, which is 1.5% of a 100 ms tick (SIM-002). No figure
is claimed for a CPU without those extensions.

### NaN

The benchmark prints the bits the machine gives an invalid operation at run
time: `0.0/0.0`, `inf-inf`, and `sqrt(-1)` all give `0xfff8000000000000` on
this x86_64 machine, while the constant `f64::NAN` is `0x7ff8000000000000`.
The sign differs from the canonical pattern, so native arithmetic and a
canonicalized plugin result hash differently unless the sim canonicalizes
(DET-022); the standard library documents the sign as non-deterministic
(DET-016), so no platform's value is a safe assumption either way.

## The aarch64 gap

When this ADR was first written no aarch64 executor was available, so it made
no claim about aarch64. Two of the points below were later measured; see
"The replay comparison" at the end of this section.
`rust-toolchain.toml` lists `aarch64-unknown-linux-gnu` and
`aarch64-apple-darwin` as targets, which covers `cargo check` only.

What is argued but not run:

- IEEE basic operations are specified to give the same value (DET-015), and
  the top-level sources of `libm`'s `sin`, `exp`, `k_sin`, `k_cos`,
  `rem_pio2`, `rem_pio2_large`, and `scalbn` contain no `fma`, `mul_add`, or
  `target_feature` reference (searched at compiler-builtins commit 9275fc9
  with `grep`). The helpers beneath them were not read. The `arch` feature of
  `libm` swaps in assembly for `sqrt`, `fma`, and rounding on aarch64 and for
  `sqrt` and `fma` on x86 (the x86 `fma` is chosen by CPU detection at run
  time), operations that are exact either way; the workspace run above had it
  on.
- Whether LLVM fuses `a * b + c` on aarch64 was not tested. The native-FMA
  run above is the nearest x86_64 evidence.

What is unknown, and why it matters:

- NaN payload and sign: the sign of the default NaN is set by the architecture
  and is believed to differ between aarch64 and x86_64 (uncited, not measured
  here), so the payload-NaN case of DET-024 must run. Measured later: the
  sign does differ on the host's native arithmetic (see below).
- Recursion depth: wasmtime documents `max_wasm_stack` as "not super-precise"
  (DET-023), and the plugin host sets a byte budget. A guest that recurses to
  the budget may trap at different call depths, hence on different ticks, on the
  two architectures if the code generators size stack frames differently. That
  was reasoning, not measurement; the depth was later measured on both hosts
  (see below).
- SHA-256 speed on aarch64 without the SHA2 extension.

What would close the gap:

1. Run `cargo nextest run -p supercom-detbench` on a real aarch64-linux machine
   and on aarch64-darwin. The golden constants committed here are the expected
   values; a failing test names the operation that differs. This closes the
   `libm`, fixed-point, hash, and basic-operation claims for that machine.
2. Build the replay comparison of DET-024 when the sim crate exists: one command
   log, replayed in CI on both x86_64 and aarch64, and once on aarch64-darwin, with
   equal digests at every tick, covering the payload-NaN and recursion-depth
   cases. This closes U1.
3. Emulation (qemu-user) would run step 1 early but is not hardware evidence for
   NaN behaviour, and the ADR will say so if it is used.

### The replay comparison

On 2026-10-18 the project ran the cross-architecture replay comparison of
DET-024 on commit `30c6874911bc07156c2ce4893e88409257e04b1b`, from a clean
checkout on each of two machines, using the comparison script's remote mode and
the harness described in `docs/adr/0015-replay-harness.md`. Both hosts use
Rust 1.99.1, and the digest program ran in release mode on each. The hosts
reported `x86_64` and `aarch64` from `uname -m`, and the result line was:

```text
local : x86_64 30c6874911bc07156c2ce4893e88409257e04b1b
remote: aarch64 30c6874911bc07156c2ce4893e88409257e04b1b
EQUAL 4611 lines
```

`EQUAL 4611 lines` means the per-tick digests of the committed skirmish log are
identical on both machines for every one of the 4611 lines. The run also covers
the payload-NaN case under the canonicalization of DET-022 and the
unbounded-recursion trap kind. Each host printed, to stderr, the raw bit
patterns of the payload-NaN case and the measured recursion depth:

| Quantity                | x86_64                                                     | aarch64                                                    |
| ----------------------- | ---------------------------------------------------------- | ---------------------------------------------------------- |
| nan host bits           | `0xfff8000000000000`                                       | `0x7ff8000000000000`                                       |
| nan plugin bits         | `0x7ff8000000000000`                                       | `0x7ff8000000000000`                                       |
| nan final position bits | `0xfff8000000000000 0x7ff8000000000000 0x406e300000000000` | `0x7ff8000000000000 0x7ff8000000000000 0x406e300000000000` |
| recursion depth         | 5454                                                       | 5452                                                       |

The two hosts' raw NaN bits differ in sign, as the argument above expected, yet
the digests are equal, which is what DET-022's canonicalization is for. The
recursion depth differs by two calls between the hosts. It is measured on each
host and recorded, not compared: DET-023 says the depth differs across
architectures, and no plugin runs in the tick loop, so the depth cannot reach a
digest. This closes the NaN and recursion points above for this one log and
these two machines; it does not cover aarch64-darwin, other compiler versions,
or logs other than the committed skirmish.

On 2026-10-24 the project ran the comparison a second time, on commit
`4f388eeca76ba6ca367d460c5feaee42191101b8`, on both hosts, checked out detached
on the aarch64 host, using the comparison script's remote mode and the harness
described in `docs/adr/0015-replay-harness.md`. The run followed a change to
the serialized army state, which now carries each army's defeat tick, its
civilian byte, and a kills counter (`docs/adr/0014-vision-and-victory.md`), and
the regeneration of the committed log to exercise it. In the regenerated log
the Cybran army is defeated at tick 4235 and the kill removes its structure at
tick 4435, within 4,600 ticks. The result line was:

```text
local : x86_64 4f388eeca76ba6ca367d460c5feaee42191101b8
remote: aarch64 4f388eeca76ba6ca367d460c5feaee42191101b8
EQUAL 4611 lines
```

The count is one line per tick of the 4,600-tick log through its last
checkpoint, plus the ten NaN-case lines and the recursion line. It is the same
number as in the first run because the log's length is unchanged, while every
tick's digest changed with the regenerated log. The NaN bit patterns and the
recursion depths, 5454 on x86_64 and 5452 on aarch64, were those of the first
run's table, so the table is not repeated. The closing sentence of the first
run still holds: the record does not cover aarch64-darwin, other compiler
versions, or logs other than the committed skirmish.

On 2026-11-27 the gate workflow gained a job on a hosted aarch64-linux runner
(`ubuntu-24.04-arm`) that runs the evidence crate's golden vectors, the
simulation crate's tests with its pinned digests of synthetic worlds, and the
texture loader's tests that need no installed game. Its first run, number
46040720557 on commit 9bb7ca1, listed 181 tests and passed 181. The five tests
that run reports as skipped were never selected: the job's filter leaves out
the four `conformance_` tests of `supercom-dds`, which ask for the installed
game, and one test of `supercom-sim` is marked ignored. Among them are the six
tests of `supercom-detbench` and the simulation's
three pinned digests (`sim_movement_golden_digest_is_pinned`,
`sim_combat_golden_digest_is_pinned`, and `sim_world_digests_golden_vector`);
the same tests with the same committed values pass on x86_64 in the gate job of
that run. This meets step 1 of "What would close the gap" for aarch64-linux and
now runs on every change. That the simulation's pinned digests agree on both
architectures is evidence the hand runs above did not have in CI. It does not
show the replay comparison of the committed skirmish on the installed map,
which still runs only by hand because CI has no installed game, and
aarch64-darwin is still unmeasured.

## Consequences

### Positive

- Blueprint-style fractional quantities stay in `f64`; no conversion layer or
  overflow budget is needed across the whole sim.
- The only platform-dependent surface is the NaN pattern, closed by one
  function (DET-022) and one hash-boundary rule.
- The strategy matches a precedent already shipped: rapier's
  `enhanced-determinism` feature selects a pure-Rust `libm` (DET-017).
- Plugin results and sim arithmetic hash alike.
- The evidence is a crate of 6 tests and two examples that run in seconds.

### Negative

- Determinism rests on the compiler never reassociating or fusing float
  operations. That is Rust's documented behaviour but not a checked invariant on
  aarch64 until step 1 above runs there.
- Every transcendental call costs a function the sim crate must route
  deliberately; a stray `.sin()` is a latent desync. The sim crate's
  `clippy.toml` forbids the standard library's transcendentals
  (`disallowed-methods`), with a fixture proving the lint fires.
- `libm`'s `sin` and `exp` are 6% and 34% slower than the platform's on this
  machine.
- U1 is not closed. The replay comparison has passed once, by hand, on two
  Linux hosts; U1 closes when it runs in CI on both architectures and once on
  aarch64-darwin. The ADR may need revising to fixed-point if the replay
  comparison fails.

---

## Alternatives Considered

| Alternative                              | Reason Rejected                                                                                                                                                                            |
| :--------------------------------------- | :----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Fixed-point throughout the sim           | Exact by construction, but every rate, range, and angle needs a format and overflow budget, and trigonometry needs tables or CORDIC. Kept as the per-quantity fallback.                    |
| Platform `std` transcendentals           | Documented as non-deterministic (DET-014); 3.1% of sine and 9.8% of exponential inputs differ from `libm` on this machine alone.                                                           |
| A software float library for all of it   | The basic operations are already exact IEEE on both architectures, so emulating them buys nothing and costs speed.                                                                         |
| Run the whole sim inside a Wasm instance | Wasm gives the same canonical NaN and IEEE operations everywhere, but puts the core loop behind the sandbox's cost (unmeasured), and ADR-0001 scopes Wasm to plugins. Revisit if U1 fails. |
| Hash only on request, not each tick      | Cheap, but a desync would be found late and the first divergent tick would be unknown (DET-011).                                                                                           |

---

## References

- AGENTS.md: R2, I2, I3, U1.
- Related ADRs: ADR-0001 (plugin boundary and NaN canonicalization), ADR-0003
  (deterministic sim and command stream).
- `docs/specs/determinism.md`: DET-009 through DET-024.
- Evidence crate: `crates/detbench`; examples `golden` and `bench`.
- Rust standard library, `f64` documentation and primitive float semantics
  (rustc 1.99.0, commit b940084):
  https://github.com/rust-lang/rust/blob/b940084d7eb6a299eb4bfeb8e34901bc051e7ac4/library/std/src/num/f64.rs
  and
  https://github.com/rust-lang/rust/blob/b940084d7eb6a299eb4bfeb8e34901bc051e7ac4/library/core/src/primitive_docs.rs
- `libm` crate, MIT: https://docs.rs/libm/0.2.16/libm/ and its source at
  https://github.com/rust-lang/compiler-builtins/tree/9275fc9ebb59ca545dac3018bf05c166617647e5/libm
- `fixed` crate, MIT or Apache-2.0: https://docs.rs/fixed/1.30.0/fixed/
- rapier `enhanced-determinism`:
  https://github.com/dimforge/rapier/blob/3406750a38286a0d3c7d0f4be2f9d53153e81d90/crates/rapier3d/Cargo.toml#L76
- Deterministic Wasm execution in wasmtime:
  https://docs.wasmtime.dev/examples-deterministic-wasm-execution.html
- wasmtime `Config::max_wasm_stack`:
  https://docs.rs/wasmtime/49.0.2/wasmtime/struct.Config.html#method.max_wasm_stack
- FAF replay parser schema, for the original's replay contents:
  https://github.com/FAForever/faf-scfa-replay-parser/blob/9d0cb7cb4bb0cf6d8be44e5c1ea4bc357aa9a0c5/replay.ksy
