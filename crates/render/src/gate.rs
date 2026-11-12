//! The skip rule of the GPU tests.

/// What a GPU-dependent test does, as [`gpu_gate`] decides.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GpuGate {
    /// An adapter exists: run the test.
    Run,
    /// No adapter and none required: print `SKIP: <reason>` and pass.
    Skip(String),
    /// No adapter but one is required: fail with this reason.
    Fail(String),
}

/// Decides what a GPU test does from whether an adapter was found and
/// whether `SUPERCOM_REQUIRE_GPU=1` is set. An adapter always runs; none
/// skips unless required, and both outcomes carry the same reason.
#[must_use]
pub fn gpu_gate(adapter_found: bool, require: bool) -> GpuGate {
    if adapter_found {
        return GpuGate::Run;
    }
    let reason = "no graphics adapter found; set SUPERCOM_REQUIRE_GPU=1 to fail instead".to_owned();
    if require {
        GpuGate::Fail(reason)
    } else {
        GpuGate::Skip(reason)
    }
}
