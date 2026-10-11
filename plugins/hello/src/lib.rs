//! The `hello` fixture: a complete plugin component for the sandbox suite of
//! `supercom-plugin`.
//!
//! It implements the `plugin` world of `wit/supercom.wit` and nothing more.
//! Built for `wasm32-unknown-unknown` and encoded with `wasm-tools`, it
//! imports only `supercom:engine/host`: `std` on that target has no I/O,
//! clock, or randomness, so no WASI interface can enter.

wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit",
});

use supercom::engine::host;

struct Hello;

impl Guest for Hello {
    fn describe() -> String {
        host::log("hello");
        String::from("hello")
    }

    fn step(tick: u64) -> u64 {
        tick
    }

    fn nan_bits() -> u64 {
        let z = core::hint::black_box(0.0f64);
        (z / z).to_bits()
    }

    fn spin() {
        loop {
            core::hint::black_box(0u8);
        }
    }
}

export!(Hello);
