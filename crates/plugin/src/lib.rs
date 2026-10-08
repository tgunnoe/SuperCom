//! The engine side of the plugin boundary: a `wasmtime` host for plugin
//! components (ADR-0001).
//!
//! A plugin is a WebAssembly component that implements the `plugin` world in
//! `wit/supercom.wit` and imports nothing but the engine's own
//! `supercom:engine/host` interface. A [`Host`] compiles components and
//! instantiates each as a [`Plugin`] under a configuration that keeps a match
//! deterministic: `NaN`s are canonicalized, relaxed SIMD is deterministic, and
//! every call runs on a fuel budget, so a guest that loops forever is stopped
//! (AGENTS.md I3).
//!
//! This is a skeleton. Every function returns [`Error::Unimplemented`]; the
//! sandbox suite in `tests/sandbox.rs` fails against it and says what the
//! host must do.

use std::fmt;

/// Why a component could not be loaded or run.
#[derive(Debug)]
pub enum Error {
    /// The bytes do not compile as a component under the host's
    /// configuration.
    Compile {
        /// What the compiler reported.
        reason: String,
    },
    /// The component compiled but could not be instantiated, for example
    /// because it imports an interface the host does not provide.
    Instantiate {
        /// What the linker or the instantiation reported.
        reason: String,
    },
    /// The guest trapped for a reason other than running out of fuel.
    Trap {
        /// What the trap reported.
        reason: String,
    },
    /// The guest used all the fuel it was given, which is how the host stops
    /// a guest that does not return. Exhaustion is deterministic: the same
    /// component and the same fuel always stop at the same point.
    OutOfFuel,
    /// The bytes are a core WebAssembly module, which has not been encoded as
    /// a component.
    NotAComponent,
    /// Skeleton only: the host is not written yet. The implementer deletes
    /// this variant.
    Unimplemented,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compile { reason } => write!(f, "component does not compile: {reason}"),
            Self::Instantiate { reason } => {
                write!(f, "component cannot be instantiated: {reason}")
            }
            Self::Trap { reason } => write!(f, "guest trapped: {reason}"),
            Self::OutOfFuel => f.write_str("guest ran out of fuel"),
            Self::NotAComponent => {
                f.write_str("not a component: the bytes are a core WebAssembly module")
            }
            Self::Unimplemented => f.write_str("not implemented"),
        }
    }
}

impl std::error::Error for Error {}

/// The engine's side of the boundary: compiles components under the
/// deterministic, fuel-metered configuration and instantiates them.
pub struct Host {
    _private: (),
}

#[expect(
    unused_variables,
    reason = "skeleton: no function reads its arguments yet"
)]
impl Host {
    /// Builds a host whose engine canonicalizes `NaN`s, makes relaxed SIMD
    /// deterministic, and meters fuel.
    ///
    /// # Errors
    ///
    /// Fails if the engine cannot be configured. The skeleton always returns
    /// [`Error::Unimplemented`].
    pub fn new() -> Result<Self, Error> {
        Err(Error::Unimplemented)
    }

    /// Compiles `bytes` as a component and instantiates it, linking only the
    /// `supercom:engine/host` interface.
    ///
    /// # Errors
    ///
    /// [`Error::NotAComponent`] if `bytes` is a core module,
    /// [`Error::Compile`] if it is not valid, [`Error::Instantiate`] if the
    /// component imports anything the host does not provide. The skeleton
    /// always returns [`Error::Unimplemented`].
    pub fn load(&self, bytes: &[u8]) -> Result<Plugin, Error> {
        Err(Error::Unimplemented)
    }

    /// Lists the fully qualified names of the interfaces a component imports,
    /// for example `supercom:engine/host@0.1.0`, without instantiating it.
    ///
    /// # Errors
    ///
    /// The same as [`Host::load`] for bytes that are not a component. The
    /// skeleton always returns [`Error::Unimplemented`].
    pub fn imports(&self, bytes: &[u8]) -> Result<Vec<String>, Error> {
        Err(Error::Unimplemented)
    }
}

/// One instance of a plugin component, with its own state and fuel.
pub struct Plugin {
    _private: (),
}

#[expect(
    unused_variables,
    reason = "skeleton: no function reads its arguments yet"
)]
impl Plugin {
    /// Calls the plugin's `describe` export.
    ///
    /// # Errors
    ///
    /// [`Error::OutOfFuel`] if the call exhausts its fuel, [`Error::Trap`] if
    /// the guest traps otherwise. The skeleton always returns
    /// [`Error::Unimplemented`].
    pub fn describe(&mut self) -> Result<String, Error> {
        Err(Error::Unimplemented)
    }

    /// Calls the plugin's `step` export for `tick`, allowing the call `fuel`.
    ///
    /// # Errors
    ///
    /// As for [`Plugin::describe`].
    pub fn step(&mut self, tick: u64, fuel: u64) -> Result<u64, Error> {
        Err(Error::Unimplemented)
    }

    /// Calls the plugin's `nan-bits` export, allowing the call `fuel`.
    ///
    /// # Errors
    ///
    /// As for [`Plugin::describe`].
    pub fn nan_bits(&mut self, fuel: u64) -> Result<u64, Error> {
        Err(Error::Unimplemented)
    }

    /// Calls the plugin's `spin` export, which never returns on its own,
    /// allowing the call `fuel`.
    ///
    /// # Errors
    ///
    /// [`Error::OutOfFuel`] once the fuel is used up, which is the only way
    /// the call ends. The skeleton always returns [`Error::Unimplemented`].
    pub fn spin(&mut self, fuel: u64) -> Result<(), Error> {
        Err(Error::Unimplemented)
    }
}
