//! Paths the sandbox suite in `tests/sandbox.rs` does not reach: the error
//! mappings other than fuel exhaustion and `NotAComponent` through `load`, the
//! order in which `imports` reports, and two configuration choices the suite
//! leaves unpinned, deterministic relaxed SIMD and the threads proposal being
//! off. They run against small components written in text, so they need
//! neither the fixture nor a build tool.
//!
//! The rest pin the limits that hold a hostile component. Each runs a crafted
//! component that a host without the limit would accept, and checks that the
//! host stops it.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::{
    COMPONENT_MAX_BYTES, Error, HOSTCALL_FUEL, Host, MAX_INSTANCES, MAX_MEMORIES, MAX_TABLES,
    MAX_WASM_STACK_BYTES, MEMORY_LIMIT_BYTES, Plugin, TABLE_ELEMENTS,
};

/// A component that implements the whole `plugin` world: `describe` answers
/// `hi`, `spin` never returns, and the bodies of `step` and `nan-bits` are
/// left to the test.
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

/// The eight bytes of an empty core module, which is all a core module needs
/// to start with.
const CORE_MODULE: &[u8] = b"\0asm\x01\0\0\0";

fn assemble(text: &str) -> Vec<u8> {
    wat::parse_str(text).expect("the test component assembles")
}

/// [`WORLD`] with the given bodies for `step` and `nan-bits`.
fn world(step: &str, nan_bits: &str) -> Vec<u8> {
    assemble(
        &WORLD
            .replace("{step}", step)
            .replace("{nan_bits}", nan_bits),
    )
}

fn host() -> Host {
    Host::new().expect("Host::new")
}

#[test]
fn bytes_that_are_not_wasm_fail_to_compile() {
    let host = host();
    let loaded = host.load(b"not wasm").err();
    assert!(
        matches!(loaded, Some(Error::Compile { .. })),
        "load returned {loaded:?}"
    );
    let listed = host.imports(&[]).err();
    assert!(
        matches!(listed, Some(Error::Compile { .. })),
        "imports returned {listed:?}"
    );
}

#[test]
fn imports_rejects_a_core_module_like_load() {
    let listed = host().imports(CORE_MODULE).err();
    assert!(
        matches!(listed, Some(Error::NotAComponent)),
        "imports returned {listed:?}"
    );
}

#[test]
fn an_import_the_host_does_not_provide_fails_to_instantiate() {
    let bytes = assemble(r#"(component (import "supercom:engine/clock@0.1.0" (instance)))"#);
    let host = host();
    assert_eq!(
        host.imports(&bytes).expect("imports"),
        ["supercom:engine/clock@0.1.0"]
    );
    let loaded = host.load(&bytes).err();
    assert!(
        matches!(loaded, Some(Error::Instantiate { .. })),
        "load returned {loaded:?}"
    );
}

#[test]
fn a_component_that_lacks_the_world_fails_to_instantiate() {
    let loaded = host().load(&assemble("(component)")).err();
    assert!(
        matches!(&loaded, Some(Error::Instantiate { reason }) if reason.contains("describe")),
        "load returned {loaded:?}"
    );
}

#[test]
fn imports_are_listed_in_declaration_order() {
    let bytes = assemble(
        r#"(component
             (import "zeta:pkg/b@0.1.0" (instance))
             (import "alpha:pkg/a@0.1.0" (instance)))"#,
    );
    assert_eq!(
        host().imports(&bytes).expect("imports"),
        ["zeta:pkg/b@0.1.0", "alpha:pkg/a@0.1.0"]
    );
}

#[test]
fn a_trap_other_than_fuel_exhaustion_is_a_trap() {
    let bytes = world("unreachable", "(i64.const 0)");
    let mut plugin = host().load(&bytes).expect("load the trapping component");
    assert_eq!(plugin.describe().expect("describe"), "hi");
    let stepped = plugin.step(1, 1_000_000);
    assert!(
        matches!(&stepped, Err(Error::Trap { reason }) if reason.contains("unreachable")),
        "step returned {stepped:?}"
    );
}

/// Relaxed SIMD leaves some results to the platform: truncating a `NaN` lane
/// gives `INT_MIN` on `x86_64` and 0 on `aarch64`. With the deterministic option
/// on, both give 0.
#[test]
fn relaxed_simd_is_deterministic() {
    let bytes = world(
        "(local.get 0)",
        "(i64.extend_i32_u (i32x4.extract_lane 0 \
           (i32x4.relaxed_trunc_f32x4_s (f32x4.splat (f32.const nan)))))",
    );
    let mut plugin = host()
        .load(&bytes)
        .expect("load the relaxed SIMD component");
    assert_eq!(plugin.nan_bits(1_000_000).expect("nan_bits"), 0);
}

/// A guest that waits on a shared memory blocks its thread beyond the reach
/// of fuel, so the threads proposal is off and such a component is refused.
#[test]
fn shared_memory_does_not_compile() {
    let bytes = assemble("(component (core module (memory 1 1 shared)))");
    let loaded = host().load(&bytes).err();
    assert!(
        matches!(loaded, Some(Error::Compile { .. })),
        "load returned {loaded:?}"
    );
}

/// One page of linear memory.
const PAGE: usize = 65_536;

/// Free stack a test thread keeps, beyond the guest's budget, for the host's
/// own frames, which are large in a debug build.
const HOST_STACK_HEADROOM: usize = 192 * 1024;

/// A component whose `step` hands the host a string of `tick` bytes through
/// `log`, and whose `describe` answers a string of `{describe_len}` bytes.
/// Both strings are zeros in a two-page memory, so either can be longer than
/// the host will lift.
const LOGGING_WORLD: &str = r#"
    (component
      (import "supercom:engine/host@0.1.0"
        (instance $host (export "log" (func (param "message" string)))))
      (core module $provider (memory (export "memory") 2))
      (core instance $memory (instantiate $provider))
      (alias export $host "log" (func $log))
      (core func $lowered (canon lower (func $log) (memory $memory "memory")))
      (core module $m
        (import "host" "log" (func $log (param i32 i32)))
        (import "env" "memory" (memory 2))
        (data (i32.const 8) "hi")
        (func (export "describe") (result i32)
          (i32.store (i32.const 0) (i32.const 8))
          (i32.store (i32.const 4) (i32.const {describe_len}))
          (i32.const 0))
        (func (export "step") (param i64) (result i64)
          (call $log (i32.const 0) (i32.wrap_i64 (local.get 0)))
          (local.get 0))
        (func (export "nan-bits") (result i64) (i64.const 0))
        (func (export "spin") (loop (br 0))))
      (core instance $i (instantiate $m
        (with "host" (instance (export "log" (func $lowered))))
        (with "env" (instance $memory))))
      (func (export "describe") (result string)
        (canon lift (core func $i "describe") (memory $memory "memory")))
      (func (export "step") (param "tick" u64) (result u64)
        (canon lift (core func $i "step")))
      (func (export "nan-bits") (result u64)
        (canon lift (core func $i "nan-bits")))
      (func (export "spin")
        (canon lift (core func $i "spin"))))
"#;

/// [`LOGGING_WORLD`] whose `describe` answers `describe_len` bytes.
fn logging_world(describe_len: usize) -> Vec<u8> {
    assemble(&LOGGING_WORLD.replace("{describe_len}", &describe_len.to_string()))
}

/// A size as the `u64` that `step` takes.
fn count(n: usize) -> u64 {
    u64::try_from(n).expect("the count fits in 64 bits")
}

/// Why `load` refuses the component written in `text`, or `loaded`.
fn refusal(text: &str) -> String {
    host()
        .load(&assemble(text))
        .err()
        .map_or_else(|| String::from("loaded"), |err| err.to_string())
}

/// A component whose one core module defines `count` copies of `item`.
fn module_with(item: &str, count: usize) -> String {
    format!(
        "(component (core module $m {}) (core instance (instantiate $m)))",
        item.repeat(count)
    )
}

/// A component that instantiates an empty core module `count` times.
fn instances(count: usize) -> String {
    format!(
        "(component (core module $m) {})",
        "(core instance (instantiate $m))".repeat(count)
    )
}

/// Runs `work` on its own thread and fails the test, rather than hanging it,
/// if it has not finished within `limit`: what these tests defend against is a
/// call that never returns.
fn returns_within<T: Send + 'static>(
    limit: Duration,
    work: impl FnOnce() -> T + Send + 'static,
) -> T {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || sender.send(work()));
    receiver
        .recv_timeout(limit)
        .expect("the call returned within the time limit")
}

/// A valid, empty component padded with a custom section to exactly `total`
/// bytes. The section's size is a LEB128 padded to five bytes, so the length
/// of the header does not depend on the size.
fn component_of_len(total: usize) -> Vec<u8> {
    let mut bytes = assemble("(component)");
    let size = total - bytes.len() - 1 - 5;
    bytes.push(0);
    for index in 0..5 {
        let group = u8::try_from((size >> (7 * index)) & 0x7f).expect("seven bits");
        bytes.push(if index < 4 { group | 0x80 } else { group });
    }
    bytes.push(7);
    bytes.extend_from_slice(b"padding");
    bytes.resize(total, 0);
    bytes
}

/// A start function that never returns is stopped by the instantiation
/// budget. Without one `load` never returns, so it runs on its own thread and
/// a missed budget fails the test instead of hanging it.
#[test]
fn a_start_function_that_never_returns_runs_out_of_fuel() {
    let bytes = assemble(
        r"(component
            (core module $m (func $spin (loop (br 0))) (start $spin))
            (core instance (instantiate $m)))",
    );
    let loaded = returns_within(Duration::from_secs(30), move || host().load(&bytes).err());
    assert!(
        matches!(loaded, Some(Error::OutOfFuel)),
        "load returned {loaded:?}"
    );
}

/// The limit is the host's, not the guest's declared maximum: this memory has
/// none. Growing to exactly the limit works and one page more answers -1.
#[test]
fn memory_stops_growing_at_the_limit() {
    let bytes = world(
        "(i64.extend_i32_s (memory.grow (i32.wrap_i64 (local.get 0))))",
        "(i64.const 0)",
    );
    let mut plugin = host().load(&bytes).expect("load the growing component");
    let limit = count(MEMORY_LIMIT_BYTES / PAGE);
    // The memory starts at one page, and `memory.grow` answers the old size.
    assert_eq!(
        plugin
            .step(limit - 1, 1_000_000)
            .expect("grow to the limit"),
        1
    );
    assert_eq!(
        plugin.step(1, 1_000_000).expect("grow past the limit"),
        u64::MAX
    );
}

#[test]
fn a_memory_declared_over_the_limit_does_not_instantiate() {
    let pages = MEMORY_LIMIT_BYTES / PAGE + 1;
    let refused = refusal(&module_with(&format!("(memory {pages})"), 1));
    assert!(
        refused.contains("memory minimum size"),
        "load refused with: {refused}"
    );
}

#[test]
fn a_table_declared_over_the_limit_does_not_instantiate() {
    let refused = refusal(&module_with(
        &format!("(table {} funcref)", TABLE_ELEMENTS + 1),
        1,
    ));
    assert!(
        refused.contains("table minimum size"),
        "load refused with: {refused}"
    );
}

#[test]
fn a_store_holds_at_most_the_allowed_tables() {
    let refused = refusal(&module_with("(table 1 funcref)", MAX_TABLES + 1));
    assert!(
        refused.contains("table count"),
        "load refused with: {refused}"
    );
    let allowed = refusal(&module_with("(table 1 funcref)", MAX_TABLES));
    assert!(
        !allowed.contains("table count"),
        "load refused with: {allowed}"
    );
}

#[test]
fn a_store_holds_at_most_the_allowed_memories() {
    let refused = refusal(&module_with("(memory 1)", MAX_MEMORIES + 1));
    assert!(
        refused.contains("memory count"),
        "load refused with: {refused}"
    );
    let allowed = refusal(&module_with("(memory 1)", MAX_MEMORIES));
    assert!(
        !allowed.contains("memory count"),
        "load refused with: {allowed}"
    );
}

#[test]
fn a_store_holds_at_most_the_allowed_instances() {
    let refused = refusal(&instances(MAX_INSTANCES + 1));
    assert!(
        refused.contains("instance count"),
        "load refused with: {refused}"
    );
    let allowed = refusal(&instances(MAX_INSTANCES));
    assert!(
        !allowed.contains("instance count"),
        "load refused with: {allowed}"
    );
}

/// Guest code runs on the caller's stack, so the pinned budget is what keeps
/// a guest that recurses without bound from overrunning the thread, which
/// aborts the process. The thread has the budget and a little more: a guest
/// allowed the 512 KiB `wasmtime` defaults to would run off the end of it.
#[test]
fn unbounded_recursion_traps_within_the_stack_limit() {
    let bytes = world("(call $step (local.get 0))", "(i64.const 0)");
    let mut plugin = host().load(&bytes).expect("load the recursing component");
    let stepped = thread::Builder::new()
        .stack_size(MAX_WASM_STACK_BYTES + HOST_STACK_HEADROOM)
        .spawn(move || plugin.step(0, u64::MAX))
        .expect("spawn the small-stack thread")
        .join()
        .expect("the guest stays inside its stack budget");
    assert!(
        matches!(&stepped, Err(Error::Trap { reason }) if reason.contains("call stack exhausted")),
        "step returned {stepped:?}"
    );
}

#[test]
fn log_takes_a_string_up_to_the_hostcall_budget() {
    let mut plugin = host()
        .load(&logging_world(2))
        .expect("load the logging component");
    let longest = count(HOSTCALL_FUEL);
    assert_eq!(
        plugin
            .step(longest, 1_000_000)
            .expect("log the longest string"),
        longest
    );
}

/// `wasmtime` reports a string over the hostcall budget as a trap raised in
/// the guest's call, so the host never copies it and the caller sees
/// [`Error::Trap`], neither a compile nor an instantiation error.
#[test]
fn log_refuses_a_string_over_the_hostcall_budget() {
    let mut plugin = host()
        .load(&logging_world(2))
        .expect("load the logging component");
    let stepped = plugin.step(count(HOSTCALL_FUEL) + 1, 1_000_000);
    assert!(
        matches!(&stepped, Err(Error::Trap { reason }) if reason.contains("hostcalls")),
        "step returned {stepped:?}"
    );
}

/// The budget holds the other way too: the host lifts the text `describe`
/// answers under the same limit.
#[test]
fn describe_text_is_held_to_the_hostcall_budget() {
    let mut longest = host()
        .load(&logging_world(HOSTCALL_FUEL))
        .expect("load the longest describe");
    let described = longest.describe().map(|text| text.len());
    assert!(
        matches!(described, Ok(len) if len == HOSTCALL_FUEL),
        "describe returned {described:?}"
    );
    let mut over = host()
        .load(&logging_world(HOSTCALL_FUEL + 1))
        .expect("load the over-long describe");
    let described = over.describe().map(|text| text.len());
    assert!(
        matches!(&described, Err(Error::Trap { .. })),
        "describe returned {described:?}"
    );
}

#[test]
fn a_component_over_the_size_limit_is_refused_before_it_compiles() {
    let host = host();
    let over = component_of_len(COMPONENT_MAX_BYTES + 1);
    let size = over.len().to_string();
    for refused in [host.load(&over).err(), host.imports(&over).err()] {
        assert!(
            matches!(&refused, Some(Error::Compile { reason }) if reason.contains(&size)),
            "refused with {refused:?}"
        );
    }
    let exact = component_of_len(COMPONENT_MAX_BYTES);
    let listed = host.imports(&exact).map(|names| names.len());
    assert!(
        matches!(listed, Ok(0)),
        "a component of exactly the limit is compiled, but imports returned {listed:?}"
    );
}

/// Asserts that no component written in `candidates` gets past compiling. Each
/// is one use of a proposal the host pins off, and `wasmtime` would accept it
/// with its defaults: it would then be refused only for lacking the world.
fn none_compile(candidates: &[&str]) {
    let accepted: Vec<_> = candidates
        .iter()
        .filter(|text| {
            !matches!(
                host().load(&assemble(text)).err(),
                Some(Error::Compile { .. })
            )
        })
        .collect();
    assert!(
        accepted.is_empty(),
        "these components were not refused at compile time: {accepted:?}"
    );
}

#[test]
fn memory64_does_not_compile() {
    none_compile(&["(component (core module (memory i64 1)))"]);
}

#[test]
fn garbage_collected_types_do_not_compile() {
    none_compile(&[
        "(component (core module (type (struct (field i32)))))",
        "(component (core module (type (array i8))))",
    ]);
}

#[test]
fn exception_handling_does_not_compile() {
    none_compile(&["(component (core module (tag (param i32))))"]);
}

#[test]
fn the_async_abi_does_not_compile() {
    none_compile(&[
        "(component (core func (canon waitable-set.new)))",
        "(component (type (future u8)))",
        "(component (type (stream u8)))",
    ]);
}

/// Asserts that every export that tells a refusal from a run is refused:
/// `describe` and `nan-bits` would answer, and `spin` would run out of fuel,
/// if the guest ran at all.
fn assert_retired(plugin: &mut Plugin) {
    let described = plugin.describe();
    assert!(
        matches!(&described, Err(Error::Trap { .. })),
        "describe returned {described:?}"
    );
    let nan_bits = plugin.nan_bits(1_000_000);
    assert!(
        matches!(&nan_bits, Err(Error::Trap { .. })),
        "nan_bits returned {nan_bits:?}"
    );
    let spun = plugin.spin(1_000);
    assert!(
        matches!(&spun, Err(Error::Trap { .. })),
        "spin returned {spun:?}"
    );
}

/// A plugin fails closed: once a call has returned an error, `wasmtime`
/// refuses to enter the instance again and the host reports the refusal as
/// [`Error::Trap`]. Its `step` here is the identity, so an error from it can
/// only be the refusal. The behaviour is `wasmtime`'s, so this test passes
/// without the host doing anything; it holds the documented guarantee in place
/// against a change of `wasmtime`.
#[test]
fn a_plugin_that_ran_out_of_fuel_is_retired() {
    let mut plugin = host()
        .load(&world("(local.get 0)", "(i64.const 0)"))
        .expect("load the component");
    let spun = plugin.spin(1_000);
    assert!(
        matches!(spun, Err(Error::OutOfFuel)),
        "spin returned {spun:?}"
    );
    let stepped = plugin.step(1, 1_000_000);
    assert!(
        matches!(&stepped, Err(Error::Trap { .. })),
        "step returned {stepped:?}"
    );
    assert_retired(&mut plugin);
}

/// As above, for a trap other than fuel exhaustion.
#[test]
fn a_plugin_that_trapped_is_retired() {
    let mut plugin = host()
        .load(&world("unreachable", "(i64.const 0)"))
        .expect("load the trapping component");
    let stepped = plugin.step(1, 1_000_000);
    assert!(
        matches!(&stepped, Err(Error::Trap { .. })),
        "step returned {stepped:?}"
    );
    assert_retired(&mut plugin);
}
