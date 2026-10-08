//! Paths the sandbox suite in `tests/sandbox.rs` does not reach: the error
//! mappings other than fuel exhaustion and `NotAComponent` through `load`, the
//! order in which `imports` reports, and two configuration choices the suite
//! leaves unpinned, deterministic relaxed SIMD and the threads proposal being
//! off. They run against small components written in text, so they need
//! neither the fixture nor a build tool.

use super::{Error, Host};

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
        (func (export "step") (param i64) (result i64) {step})
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
