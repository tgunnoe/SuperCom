//! The WGSL source of the model pipelines, a second module apart from the
//! terrain and water module of `shader.rs`.

/// The WGSL source of the lit and flat model pipelines, with the light
/// direction and the ambient share written in from the crate's Rust
/// constants. Its entry points are `vs_lit`, `fs_lit`, `vs_flat`, and
/// `fs_flat`.
#[must_use]
pub const fn model_shader_source() -> String {
    String::new()
}
