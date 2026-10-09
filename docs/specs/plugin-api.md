# Plugin API specification

This file specifies the plugin boundary of SuperCom: the world a plugin component is written against, how that world is versioned, the sandbox the engine's host holds every component to, and the policy a loader applies around that host. The host is the crate `supercom-plugin` (`crates/plugin`), which runs components under `wasmtime`; the world is `wit/supercom.wit`. The decision to put plugins behind Wasm components is ADR-0001; the determinism and sandbox requirements are AGENTS.md R2, R3, and I3.

The key words MUST, MUST NOT, SHOULD, and MAY are to be read as in BCP 14. Each clause names its evaluator. A clause whose evaluator is a test is machine-verified: the test is one of those of `supercom-plugin` at the commit this file was written against, cited as `cargo nextest list --workspace` prints it (`sandbox_<name>` for the integration suite in `crates/plugin/tests/sandbox.rs`, `tests::<name>` for the unit tests in `crates/plugin/src/tests.rs`). A clause whose evaluator is a pinned source is a design decision of this project, checked by review and not run. Where a guarantee is implemented in the host but no test holds it, the clause says it is not pinned by a test, so that no reader takes the host's behaviour as evidenced.

Scope. The clauses fix the host-side contract: what bytes the host admits, what it refuses, what a call may consume, what a failure leaves behind, and what a loader around the host has to do with the host's answers. They do not fix a guest toolchain or compile target: admission is decided by the component bytes alone (see the clause on admission by bytes). The real interfaces a sim plugin will call (units, orders, economy) are designed by a later campaign against the simulation specification; this file fixes the world's shape rules and versioning and names those interfaces as a frontier.

Informative. The reference plugin in `plugins/hello` is the component the sandbox suite loads. The suite builds it as a core module for the `wasm32-unknown-unknown` target, then encodes the module as a component with `wasm-tools`, both found on `PATH`. That recipe is the fixture's, not a requirement on any plugin; the tests that need no build tool (`tests::*`) assemble their components from text.

The licensing statement for plugins, whether a component built against the world is a derivative work of the engine and what licence it must carry, is reserved for the head as clause PLG-001. It is not written as a clause here: the section on licensing sets out the options.

## The world and its versioning

**PLG-002** `design` `source:wit/supercom.wit`
The plugin world at version 0.1.0 MUST be the one declared in `wit/supercom.wit`: package `supercom:engine@0.1.0`, an interface `host` holding `log: func(message: string)`, and a world `plugin` that imports `host` and exports `describe: func() -> string`, `step: func(tick: u64) -> u64`, `nan-bits: func() -> u64`, and `spin: func()`. A component is a plugin of this world only if it implements every export; it MAY omit imports it does not call.

**PLG-003** `design` `source:docs/adr/0001-plugin-boundary-wasm-components.md`
The WIT world MUST be the single versioned contract between the engine and a plugin, and the host's decision to admit a component MUST depend on the component's bytes alone (their size, their validity under the pinned feature set, the imports the linker provides, the exports the world requires), never on the language, toolchain, or compile target that produced them.

**PLG-004** `design` `source:crates/plugin/src/lib.rs`
The version of the world MUST be the version of the package in `wit/supercom.wit`, and the name the host registers for its interface (the constant `HOST_INTERFACE`, `supercom:engine/host@0.1.0`) MUST carry that version. The version follows semantic versioning, read for 0.y.z as a change of y being breaking. Removing or renaming an interface, function, or export, changing a signature or a type, and adding an export to the world are breaking, because a component built earlier either imports what the host no longer defines or lacks an export the host requires; adding a function to an interface the host provides, or a new interface the host provides, is compatible.

**PLG-005** `design` `test:tests::a_component_that_lacks_the_world_fails_to_instantiate`
The host MUST look up every export of the world when it loads a component, and a component that lacks one MUST fail Host::load with Error::Instantiate whose reason names an export of the world, as the empty component's refusal names `describe`, so that the lack shows at load and not in the middle of a match.

## Admission: what the host accepts

**PLG-006** `design` `test:tests::bytes_that_are_not_wasm_fail_to_compile`
Host::load of bytes that are not WebAssembly (the test passes the text `not wasm`), and Host::imports of the empty slice, MUST fail with Error::Compile.

**PLG-007** `design` `test:sandbox_core_module_is_not_a_component`
Host::load of a valid core WebAssembly module that has not been encoded as a component MUST fail with Error::NotAComponent, not with Error::Compile, so that an author who skipped the encoding step is told so.

**PLG-008** `design` `test:tests::imports_rejects_a_core_module_like_load`
Host::imports of a core module MUST fail with Error::NotAComponent, as Host::load does.

**PLG-009** `design` `test:tests::a_component_over_the_size_limit_is_refused_before_it_compiles`
Host::load and Host::imports MUST refuse an input longer than `COMPONENT_MAX_BYTES` (16 MiB, 16,777,216 bytes) with Error::Compile whose reason states the input's length, although the input is a valid component, and MUST NOT refuse an input of exactly that length for its size.

**PLG-010** `design` `test:sandbox_component_imports_only_the_engine_host`
Every top-level import of a plugin MUST be an interface of its world, which for world 0.1.0 is the single name `supercom:engine/host@0.1.0`, and no `wasi:` interface is among them; the reference plugin's import list, as Host::imports reports it, is exactly that one name.

**PLG-011** `design` `test:tests::imports_are_listed_in_declaration_order`
Host::imports MUST list the imports of a component in the order the component declares them, not sorted: a component declaring `zeta:pkg/b@0.1.0` before `alpha:pkg/a@0.1.0` is listed in that order.

**PLG-012** `design` `test:tests::an_import_the_host_does_not_provide_fails_to_instantiate`
A component that imports an interface the host does not provide (the test imports `supercom:engine/clock@0.1.0`) MUST fail Host::load with Error::Instantiate even though Host::imports lists that name: the linker refuses what the host does not define, and the import list does not.

**PLG-013** `design` `test:tests::wasi_imports_fail_to_instantiate`
A component that otherwise implements the whole world but imports a `wasi:` instance carrying a function (the test adds `wasi:clocks/monotonic-clock@0.2.0`, `wasi:random/random@0.2.0`, and `wasi:filesystem/types@0.2.0` in turn) MUST fail Host::load with Error::Instantiate, while the same component without that import loads; so that no clock, source of randomness, or file system can reach a plugin (AGENTS.md I3).

**PLG-014** `design` `test:sandbox_describe_returns_the_plugins_text`
The host's linker MUST define the interface `supercom:engine/host` so that a plugin that calls its `log` function loads, and Plugin::describe MUST return the plugin's own text (`hello` for the reference plugin, which calls `log` on the way).

**PLG-015** `design` `test:sandbox_step_within_fuel_returns_the_result`
Plugin::step MUST return the guest's result unchanged when the call stays within its fuel: the reference plugin's `step` echoes its tick, and a tick of 7 with 1,000,000 fuel returns 7.

## Fuel

**PLG-016** `design` `test:sandbox_spin_runs_out_of_fuel`
A guest call that does not return MUST be stopped by fuel metering and MUST fail with Error::OutOfFuel, not hang and not fail as Error::Trap: Plugin::spin with 10,000 fuel fails with Error::OutOfFuel.

**PLG-017** `design` `test:sandbox_fuel_exhaustion_is_deterministic`
Two fresh loads of one component, each given the same fuel, MUST both end the call with Error::OutOfFuel. The test does not observe the point in the guest at which either stops, and it does not call a plugin again after an error (see the retirement clauses).

**PLG-018** `design` `test:tests::a_start_function_that_never_returns_runs_out_of_fuel`
Instantiation runs guest code, so it MUST run on a fuel budget of its own (`INSTANTIATE_FUEL`, 10,000,000), and a component whose start function never returns MUST make Host::load fail with Error::OutOfFuel in bounded time.

**PLG-019** `design` `test:tests::a_trap_other_than_fuel_exhaustion_is_a_trap`
A guest trap other than fuel exhaustion MUST surface from the call as Error::Trap carrying the trap's reason (the test's `unreachable` instruction reports a reason containing `unreachable`), and MUST NOT be reported as Error::OutOfFuel.

**PLG-020** `design` `source:crates/plugin/src/lib.rs`
Every call into a plugin MUST start from exactly the fuel it is given and never from fuel left over from an earlier call, and Plugin::describe, which takes no budget, MUST run on `DESCRIBE_FUEL` (1,000,000). Both come from the single function through which every call passes. Neither guarantee is pinned by a test: no test calls one plugin twice with different budgets, and none measures how much fuel `describe` is given.

## Arithmetic

**PLG-021** `design` `test:sandbox_nan_bits_are_canonical`
A `NaN` that a guest's float arithmetic produces MUST reach the host in the canonical quiet form: the reference plugin's `nan-bits`, which divides zero by zero as an `f64`, MUST return `0x7ff8000000000000` (sign clear, only the top payload bit set), so that two platforms replaying one command stream cannot diverge on it (AGENTS.md R2). A `NaN` a guest builds without arithmetic, by a sign operation (`neg`, `abs`, `copysign`), a reinterpretation of integer bits, a constant, or a load, keeps its bits, which WebAssembly defines exactly and so are the same on every host; that pass-through is not pinned by a test, since the cited test divides zero by zero and builds no such `NaN`.

**PLG-022** `design` `test:tests::relaxed_simd_is_deterministic`
Relaxed SIMD MUST be deterministic: `i32x4.relaxed_trunc_f32x4_s` of a `NaN` lane, which gives `INT_MIN` on x86_64 and 0 on aarch64 when the platform decides, MUST give 0 in a plugin.

## Resource limits

**PLG-023** `design` `test:tests::memory_stops_growing_at_the_limit`
A linear memory MUST NOT grow past `MEMORY_LIMIT_BYTES` (64 MiB, 1,024 pages) even when the guest declares no maximum: a guest that declares none and grows from one page to exactly 1,024 pages succeeds, and one page more answers -1.

**PLG-024** `design` `test:tests::a_memory_declared_over_the_limit_does_not_instantiate`
A component that declares a memory whose minimum is over `MEMORY_LIMIT_BYTES` (1,025 pages in the test) MUST fail Host::load, and the refusal MUST name the memory's minimum size.

**PLG-025** `design` `test:tests::a_table_declared_over_the_limit_does_not_instantiate`
A component that declares a table whose minimum is over `TABLE_ELEMENTS` (10,000; 10,001 in the test) MUST fail Host::load, and the refusal MUST name the table's minimum size.

**PLG-026** `design` `test:tests::a_store_holds_at_most_the_allowed_memories`
A plugin MUST NOT hold more than `MAX_MEMORIES` (4) linear memories: a component that defines five fails Host::load with a refusal naming the memory count, and one that defines four is not refused for it.

**PLG-027** `design` `test:tests::a_store_holds_at_most_the_allowed_tables`
A plugin MUST NOT hold more than `MAX_TABLES` (4) tables: a component that defines five fails Host::load with a refusal naming the table count, and one that defines four is not refused for it.

**PLG-028** `design` `test:tests::a_store_holds_at_most_the_allowed_instances`
A plugin MUST NOT hold more than `MAX_INSTANCES` (16) instances: a component that instantiates a module 17 times fails Host::load with a refusal naming the instance count, and one that does it 16 times is not refused for it.

**PLG-029** `design` `test:tests::unbounded_recursion_traps_within_the_stack_limit`
Guest code MUST NOT use more than `MAX_WASM_STACK_BYTES` (256 KiB) of the calling thread's native stack: a guest that recurses without bound fails with Error::Trap whose reason reports an exhausted call stack, on a thread whose stack is that budget plus 192 KiB, without overrunning the thread.

**PLG-030** `design` `test:tests::log_takes_a_string_up_to_the_hostcall_budget`
One crossing of the boundary MUST lift at most `HOSTCALL_FUEL` (64 KiB, 65,536 bytes) of data from the guest, and a string of exactly that length handed to `log` MUST be accepted.

**PLG-031** `design` `test:tests::log_refuses_a_string_over_the_hostcall_budget`
A string of 65,537 bytes handed to `log` MUST fail the guest's call with Error::Trap whose reason mentions hostcalls, so that the host never copies it. The mapping to Error::Trap is the choice `wasmtime` makes for a string over the budget; the test holds it in place against a change of `wasmtime`.

**PLG-032** `design` `test:tests::describe_text_is_held_to_the_hostcall_budget`
The text Plugin::describe returns MUST be held to the same budget: a `describe` that answers 65,536 bytes returns them, and one that answers 65,537 bytes fails with Error::Trap.

## The pinned feature set

**PLG-033** `design` `test:tests::shared_memory_does_not_compile`
The threads proposal MUST be off: a component with a shared memory MUST fail Host::load with Error::Compile, because a guest that waits on a shared memory blocks its thread where fuel cannot reach it.

**PLG-034** `design` `test:tests::memory64_does_not_compile`
The 64-bit memory proposal MUST be off: a component with a 64-bit memory MUST fail Host::load with Error::Compile.

**PLG-035** `design` `test:tests::garbage_collected_types_do_not_compile`
The garbage collection proposal MUST be off: components that declare a struct type and an array type MUST fail Host::load with Error::Compile.

**PLG-036** `design` `test:tests::exception_handling_does_not_compile`
The exception handling proposal MUST be off: a component that declares a tag MUST fail Host::load with Error::Compile.

**PLG-037** `design` `test:tests::the_async_abi_does_not_compile`
The component model's async ABI MUST be off: components that use a waitable set, a future type, and a stream type MUST fail Host::load with Error::Compile, because the async ABI schedules guest tasks beyond the fuel of any one call.

**PLG-038** `design` `source:crates/plugin/src/lib.rs#config`
The engine configuration MUST pin the Cranelift compiler (so that a change of the default compiler cannot drop `NaN` canonicalization), canonicalize the `NaN` results of float arithmetic (PLG-021), make relaxed SIMD deterministic, meter fuel, pin the native stack budget, turn threads, 64-bit memories, garbage collection, exception handling, and the async ABI off (PLG-033 to PLG-037), and turn backtrace details off so that the text of a trap does not depend on the environment or on debug information the guest supplies. Seven proposals stay at the defaults of `wasmtime` 49, which admit them: tail calls, function references, wide arithmetic, SIMD, relaxed SIMD, multi-memory, and extended-const. A `wasmtime` upgrade that changes the default of any of the seven widens or narrows what a guest may use without a diff in the configuration, so such an upgrade MUST re-check the set against this clause and amend it in the same change. No test pins the choice of the Cranelift compiler or the backtrace setting, and no test shows that the seven proposals are admitted, so those parts of this clause are not pinned by a test.

## Failing closed

**PLG-039** `design` `test:tests::a_plugin_that_ran_out_of_fuel_is_retired`
After a call has failed with Error::OutOfFuel, the plugin MUST be retired: every later call (`step`, `describe`, `nan-bits`, `spin`) MUST fail with Error::Trap and MUST NOT run guest code.

**PLG-040** `design` `test:tests::a_plugin_that_trapped_is_retired`
After a call has failed with a guest trap, the plugin MUST be retired in the same way as after PLG-039. The behaviour is that of `wasmtime`, which refuses to enter an instance again after an error; the test holds it in place against a change of `wasmtime`.

**PLG-041** `design` `source:crates/plugin/src/lib.rs#Plugin`
A plugin MUST be retired after any call that returns an error, including a call whose guest overran its stack, passed `log` a string over the budget, or answered `describe` with text over the budget, because what the guest left behind cannot be trusted; a fresh instance requires loading the component again. Retirement after the errors of PLG-039 and PLG-040 is pinned by those clauses' tests; retirement after the other errors follows from the same mechanism and is not pinned by a test, since no test calls a plugin again after an oversize string or an exhausted stack.

## The loader's policy

A loader is the code around the host that decides which components a match may use, when they are compiled, and what happens to a plugin after an error. The host's guarantees above are conditions on a single call; the clauses here are conditions on what a loader may infer from the host's answers.

**PLG-042** `design` `source:crates/plugin/src/lib.rs#Host`
The linker, not the import list, MUST be the gate on what a component may call. Host::imports compiles the component and reports the names of its top-level imports as the component declares them, without consulting the linker (PLG-012), so a loader MUST NOT treat a clean list as proof that a component is admitted and MUST NOT use the list as an allowlist; Host::load, which links the component against the one interface the host defines, is the decision. A loader MAY show the list to a component's author when load refuses. That the list holds top-level names only, and enumerates neither nested instance imports nor resource types, follows from how the host reads the component's type and is not pinned by a test.

**PLG-043** `design` `source:crates/plugin/src/lib.rs#HOST_INTERFACE`
Import versions bind by semantic version, which makes the linker the judge of a declared version as well: a component that declares `supercom:engine/host@0.1.7` loads against the host's `supercom:engine/host@0.1.0` while Host::imports reports `@0.1.7`, and components that declare `@0.2.0`, `@0.0.9`, or `@1.0.0` fail Host::load with Error::Instantiate. A loader MUST read a version in an import list as the version the component declared and never as the version it is bound to, and MUST NOT use a comparison with `HOST_INTERFACE` as a compatibility check. The behaviour is that of `wasmtime`; it was witnessed at the commit this file was written against, with `wasmtime` 49.0.2, by a throwaway program that loaded the full world under each version in turn, and it is not pinned by a test.

**PLG-044** `design` `source:crates/plugin/src/lib.rs#Error`
Error::Compile has three meanings and a loader MUST NOT read it as one of them: the input is longer than `COMPONENT_MAX_BYTES` (PLG-009), the bytes do not compile as a component under the pinned feature set (PLG-006 and PLG-033 to PLG-037), or the host's engine could not be configured, which is how Host::new fails. A loader that has to tell them apart MUST compare the input's length with the limit itself before calling the host, and MUST treat a failure of Host::new as a fault of the engine and not of the component. All three refuse the component. That Host::new can fail is not pinned by a test.

**PLG-045** `design` `source:crates/plugin/src/lib.rs`
The limits of PLG-023 to PLG-028 apply to one loaded plugin (one Host::load, which gives the plugin its own store), so a loader MUST budget a plugin's linear memory as `MAX_MEMORIES` times `MEMORY_LIMIT_BYTES`, which is 4 times 64 MiB, 256 MiB, and not as 64 MiB. Likewise one plugin holds at most `MAX_TABLES` (4) tables of at most `TABLE_ELEMENTS` (10,000) elements and at most `MAX_INSTANCES` (16) instances, and a component that declares more than a cap fails Host::load. The total is arithmetic over those clauses, and no test pins it.

**PLG-046** `design` `source:crates/plugin/src/lib.rs#COMPONENT_MAX_BYTES`
Compiling a component is not metered: fuel meters guest code, and the size cap bounds only the input's length, so the time to compile an input within `COMPONENT_MAX_BYTES` is not bounded by the host. A loader MUST NOT compile an untrusted component (Host::load and Host::imports both compile) on the thread that advances ticks unless it has bounded the input below `COMPONENT_MAX_BYTES`, and SHOULD compile off that thread. The absence of a compile-time bound is not pinned by a test.

**PLG-047** `design` `source:crates/plugin/src/lib.rs#Plugin`
The text Plugin::describe returns is untrusted: the host bounds its length (PLG-032) and checks nothing else about it. A loader MUST NOT use it to decide what a plugin may do or what it is called in the sim, and a front end that shows it MUST treat it as plain text to be escaped, not as markup or a path.

**PLG-048** `design` `source:AGENTS.md`
The fuel a loader gives a call (the `fuel` argument of Plugin::step, Plugin::nan_bits, and Plugin::spin) is an input to the sim and MUST be a function of sim state and configuration alone, such as a constant or a function of the tick, never of the clock, of a measurement, or of the machine, because fuel exhaustion is how the host ends a call and machines that gave different budgets would end it at different ticks (R2, I2).

**PLG-049** `design` `source:crates/plugin/src/lib.rs#Plugin`
Guest code runs on the stack of the thread that calls it, so a loader MUST call a plugin only on a thread with at least `MAX_WASM_STACK_BYTES` (256 KiB) of free stack plus the host's own frames; on a thread with less, a guest that recurses without bound overruns the thread and aborts the process, which no Error variant reports. The 192 KiB of headroom the test of PLG-029 gives its thread in a debug build is a witness of one build and not a bound.

## Cross-architecture witnesses

The tests above run on one architecture. Two of their guarantees rest on facts that differ between x86_64 and aarch64, and the cross-architecture replay comparison named by AGENTS.md U1 (and by the determinism specification when it exists) is the evaluator that closes them.

**PLG-050** `design` `source:AGENTS.md`
Equality of `NaN` bit patterns across x86_64 and aarch64 MUST be treated as unverified. The test of PLG-021 is a witness on x86_64 only: there the hardware's `NaN` for `0.0 / 0.0` differs from the canonical pattern, so the test can fail without canonicalization, while on aarch64 the same division already gives the canonical pattern, so the test passes whether or not the host canonicalizes. A payload `NaN` that arithmetic produces on one architecture and not on the other is not covered by any test, and U1's replay comparison is to include a guest that computes one.

**PLG-051** `design` `source:AGENTS.md`
The depth at which unbounded recursion traps at `MAX_WASM_STACK_BYTES` MUST be treated as architecture-dependent and unverified, because the native frame sizes of the compiled guest differ between x86_64 and aarch64: the test of PLG-029 shows that the guest traps and stays inside its budget on one build, not at what depth. A plugin MUST NOT depend on the depth at which it traps, and until U1's replay comparison includes a recursing guest, a match that mixes architectures with a plugin whose recursion approaches the budget lies outside the verified envelope.

## The interfaces frontier

The world of version 0.1.0 is deliberately small: one host function that takes a string, and four exports over strings and integers. The interfaces a sim plugin will call (queries over units and their state, orders, the economy, the map, the tick, and whatever the AI needs; AGENTS.md U8) are designed by a later campaign against the simulation specification and are not specified here. The clauses below fix the shape rules that campaign works within.

**PLG-052** `design` `source:AGENTS.md`
A host function added to the world MUST be a deterministic function of its arguments and the sim state: it MUST NOT read the clock, OS randomness, or the file system, and MUST NOT depend on the iteration order of an unordered map (I2, I3), and the values it returns MUST NOT expose a host address, so that the plugin cannot make two replays differ.

**PLG-053** `design` `source:docs/adr/0001-plugin-boundary-wasm-components.md`
A host function added to the world MUST take and return its data by value in WIT types, referring to sim entities by handle and not by copy, and MUST bound the work the host does for one call, charging data lifted from the guest against the hostcall budget of PLG-030 and PLG-031; ADR-0001 asks that the API be designed for batching and handles rather than copies.

**PLG-054** `design` `source:AGENTS.md`
A host function other than `log` MUST NOT be added to `wit/supercom.wit` before a clause of this domain specifies it, because code for a subsystem with no clause is the signpost that defeats the spec-first requirement (R7), and the addition MUST follow the version rules of PLG-004.

## Licensing: PLG-001 options

This section is not a clause and carries no evaluator. The head has reserved the licensing statement for plugins as clause PLG-001 (R7). The identifier is unused, and the requirement check for it fails until the head rules. The options below are drafted for that ruling; none is chosen, and none is legal advice. The consequences are read from the texts cited, as they stood when this file was written, not from any court's reading of them.

### The question

ADR-0005 licenses the engine and every workspace crate under GPL-3.0-or-later, leaves open whether the plugin boundary makes a plugin a separate work, and says the plugin API specification owes modders a clear statement. ADR-0001 chose Wasm components over a WIT engine API for moddability and names no licensing statement for plugins. The question is whether a component built against the world is a derivative work of the engine, and what licence terms a plugin must carry or may choose.

### Facts that bear on every option

- A plugin is a component compiled apart from the engine and loaded at run time by the engine's `wasmtime` host. It shares no linear memory with the engine. Everything crosses by value through the typed functions of the world, and every crossing is bounded (PLG-030 to PLG-032).
- Today the crossing carries a string and integers (PLG-002). The interfaces of the frontier will carry richer data: the FSF's published test weighs the semantics of what is exchanged as well as the mechanism, so the facts change as the world grows.
- `wit/supercom.wit` is part of the GPL-licensed repository. Guest toolchains commonly generate bindings from a WIT file and compile them into the component; the reference plugin does this with `wit-bindgen`. Whether that generated code is covered by the engine's licence is a question for each option.
- ADR-0001 plans a Rust SDK crate and a project template that every Rust plugin would link. Their licence is decided by neither ADR.
- The engine's own dependencies are permissive, LGPL-3.0, or MPL-2.0 (ADR-0005 and its amendment), which is GPL-compatible and does not bear on plugins.

### References

The FSF's pages carry no commit hash; each is given by anchor and was read on 2026-10-08. The two licence texts are fixed versions.

- GNU GPL FAQ, when a program and its plug-ins form one combined program: https://www.gnu.org/licenses/gpl-faq.html#GPLPlugins
- GNU GPL FAQ, the licence terms of a plug-in for a GPL-covered program: https://www.gnu.org/licenses/gpl-faq.html#GPLAndPlugins
- GNU GPL FAQ, an interpreter extended with bindings: https://www.gnu.org/licenses/gpl-faq.html#IfInterpreterIsGPL
- GNU GPL FAQ, aggregates and the line between two programs and one program: https://www.gnu.org/licenses/gpl-faq.html#MereAggregation
- GNU GPL FAQ, an exception that allows linking over a controlled interface only: https://www.gnu.org/licenses/gpl-faq.html#LinkingOverControlledInterface
- GNU GPL version 3, 29 June 2007, section 7 (additional permissions): https://www.gnu.org/licenses/gpl-3.0.html#section7
- GNU LGPL version 3, 29 June 2007, section 4 (combined works) and section 5 (combined libraries): https://www.gnu.org/licenses/lgpl-3.0.html#section4

### Option A: a plugin is a derivative work

The statement a clause would make: a plugin component distributed for use with the engine MUST be licensed under GPL-3.0-or-later or under a licence compatible with it, and its corresponding source MUST be available to every recipient of the component.

Grounds. The FSF takes in-process dynamic linking, with function calls and shared data structures, to form one combined program (GPL FAQ, `#GPLPlugins`), and says the licence of a plug-in in a combined program must be the GPL or a GPL-compatible free licence (`#GPLAndPlugins`). Its entry on interpreters says that when an interpreter provides bindings to other facilities, the interpreted program is effectively linked to the facilities it uses through them (`#IfInterpreterIsGPL`); the host functions of the world are such bindings. The option also matches the purpose ADR-0005 records: derived engines and distributed modifications ship their source. Against it, the sandbox is a technical separation (no shared address space, by-value crossings), and the FSF itself calls the line between two programs and one a legal question that judges decide (`#MereAggregation`), so this option asserts the stricter reading of an unsettled question as the project's policy.

First-party plugins. Nothing changes: plugins in the repository carry GPL-3.0-or-later like every crate, and the SDK and template can too.

Third-party plugins. Free-software plugins, including those under permissive licences such as MIT or Apache-2.0, are admitted, and each must ship source, since a Wasm binary alone is not source. Closed-source and paid-binary plugins are excluded, as are plugins under licences that are not GPL-compatible, for example licences that forbid commercial use. This is the simplest statement and the narrowest ecosystem.

### Option B: the world is an interface boundary and plugins may carry any licence

The statement a clause would make: a plugin component that reaches the engine solely through a published version of the plugin world is a separate work, its author MAY license it under any terms, and the engine's licence notice says so as an additional permission.

Grounds. Where communication between a program and its plug-ins is limited and not intimate, the FSF treats them as separate programs (`#GPLPlugins`), and its criterion looks at the mechanism of communication and its semantics (`#MereAggregation`). The mechanism here is nearer to message passing than to a shared address space: the guest has its own linear memory, data is copied across a bounded crossing, and the host meters the guest. An explicit permission removes the doubt in the plugin author's favour whatever a court would hold; GPL-3.0 section 7 lets the copyright holders add such a permission, and the FAQ gives a model text for an exception limited to modules that communicate solely through a named interface (`#LinkingOverControlledInterface`). The option serves ADR-0001's aim of maximizing moddability.

First-party plugins. They stay in a GPL repository and MAY carry any licence the head chooses for them; nothing forces them to be GPL.

Third-party plugins. Closed-source and commercial plugins are admitted with no source obligation. For the permission to mean anything, the interface artifacts every plugin embeds, the WIT files, the generated bindings, and any SDK crate and template, must be covered by it or carry a permissive licence; otherwise the bindings compiled into each plugin would carry the GPL into it. Three costs follow. The permission is hard to withdraw for releases already made. Its wording has to say what "solely through the published world" covers, including which world versions, because a richer world may exchange the intimate data structures that the FSF's test counts against separateness, and then the permission, not the argument, carries the weight. Engine forks remain GPL.

### Option C: a conditional linking exception

The statement a clause would make: the engine remains GPL-3.0-or-later with an additional permission under section 7 that lets a component which reaches the engine solely through a published world version be licensed on terms of its author's choice, on conditions that keep changes to the interface in the commons: the interface artifacts (the WIT files, the SDK crate, and the template) are licensed LGPL-3.0-or-later, and the permission does not cover a plugin that embeds engine code from `crates/`.

Grounds. This takes the controlled-interface exception of Option B (`#LinkingOverControlledInterface`) and adds the structure of the LGPL, the established model for a library that programs under other licences may use on conditions. LGPL-3.0 section 4 lets a combined work be conveyed under terms of the author's choice if the terms do not restrict modification of the library portions and reverse engineering to debug such modifications, with notices, the licence text, and a way for the recipient to replace the library; section 5 covers library facilities placed side by side with others. The engine's copyleft stays intact, and the SDK's copyleft is limited to the SDK.

First-party plugins. Unchanged from Option B.

Third-party plugins. Closed-source plugins are admitted, but their authors take on the LGPL's conditions with respect to the interface artifacts they embed. How the replace-the-library condition is met by a statically compiled component is untested here: it might be met by supplying the SDK as a separate component that is composed in, or by shipping something relinkable, and either shapes the SDK's design. This option asks more of authors than Option B and less than Option A, and it is the most complex to word, so it should have legal review before the head adopts it.

### What the head decides

- Which option governs, or another course. Each of A, B, and C answers the question of PLG-001 differently, and the clause is written only after the ruling.
- The licence of the interface artifacts: `wit/`, the SDK crate, and the template. Options B and C depend on this choice and A is satisfied by the default.
- Where the statement lives: the `LICENSE` notice, a separate exception file, per-file headers, or all of them, with the wording for section 7 of the GPL if B or C is chosen.
- The scope of a permission over time: which world versions it covers, and whether a later, richer world needs a fresh ruling.
- Whether the loader reads a licence declaration from a component and enforces anything, which would be new surface in the host and in this specification, or whether the statement is a published policy only.
- Whether to take legal advice before ruling, and whether the ruling is recorded as an ADR beside ADR-0001 and ADR-0005, with PLG-001 as the clause that cites it.

## Questions for the head

The licensing options above are the first question. A second concerns the guest side of the boundary and is recorded here for the same ruling; it is not specified by any clause.

### Should the boundary admit WASI-built guests?

The facts, from the plugin node's record in https://github.com/tgunnoe/SuperCom/pull/11:

- A `std` guest built for `wasm32-wasip2` imports fourteen WASI 0.2 interfaces (io, clocks, and cli).
- The current fixture path, a core module built for `wasm32-unknown-unknown` and encoded as a component, keeps `std` with zero WASI imports, so such a guest loads.
- Today the linker defines no `wasi:` interface, and a component that imports one fails to load (PLG-013); AGENTS.md I3 forbids clock, random, and filesystem imports for sim-side plugins.

The choices:

- Keep the boundary as it is. A guest reaches it only through a build path that leaves no `wasi:` import, as the fixture's does. Cost: authors work with a narrower toolchain and cannot use the default `std` target for components.
- Admit WASI guests through deterministic host stubs: the host would define the `wasi:` interfaces that std guests need, backed by stubs that read no real clock, give no real randomness, and open no files, so a std plugin for `wasm32-wasip2` would load. This helps moddability. It amends the letter of I3, which forbids those imports and not only their real behaviour, and it adds to this specification a clause for each stub that fixes its determinism and a witness for it.
- A build-time step in the SDK that supplies the stubs inside the component, so that the component the host sees has no `wasi:` import. This leaves I3 and the linker as they are. Its feasibility has not been assessed here.

Until the head rules, I3 governs and the linker defines no `wasi:` interface.
