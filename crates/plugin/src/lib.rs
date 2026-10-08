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
//! The host's linker defines the `host` interface and nothing else, and the
//! crate links no system interface, so a component that imports a clock, a
//! source of randomness, or the file system compiles but does not instantiate.

use std::fmt;

use wasmtime::component::{
    Component, ComponentNamedList, Instance, Lift, Linker, Lower, TypedFunc,
};
use wasmtime::{Config, Engine, Store, Strategy, Trap};

/// The one interface the host provides to a plugin.
const HOST_INTERFACE: &str = "supercom:engine/host@0.1.0";

/// How a core WebAssembly module starts: the magic `\0asm` and binary-format
/// version 1. A component starts with the same magic and the words `0d 00` and
/// `01 00` (component-model version and layer), so these eight bytes tell the
/// two apart before any compiling.
const CORE_MODULE_PREAMBLE: [u8; 8] = [0, 0x61, 0x73, 0x6d, 1, 0, 0, 0];

/// The fuel [`Host::load`] gives a new store before instantiating. A fresh
/// store holds none, and instantiation runs guest code, so without this the
/// guest traps before it is up. It is all the fuel there is: instantiation is
/// not bounded.
const INSTANTIATE_FUEL: u64 = u64::MAX;

/// The fuel [`Plugin::describe`] runs under, since it takes no budget.
const DESCRIBE_FUEL: u64 = 1_000_000;

/// Why a component could not be loaded or run.
#[derive(Debug)]
pub enum Error {
    /// The bytes do not compile as a component under the host's
    /// configuration, or the host's compiler could not be set up.
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
        }
    }
}

impl std::error::Error for Error {}

impl Error {
    /// A compiler failure, with the whole chain of causes `wasmtime` reports.
    fn compile(err: &wasmtime::Error) -> Self {
        Self::Compile {
            reason: format!("{err:#}"),
        }
    }

    /// A linker or instantiation failure.
    fn instantiate(err: &wasmtime::Error) -> Self {
        Self::Instantiate {
            reason: format!("{err:#}"),
        }
    }

    /// What a failed guest call reported: exhausted fuel is its own variant
    /// and every other trap is a [`Error::Trap`].
    fn call(err: &wasmtime::Error) -> Self {
        if err.downcast_ref::<Trap>() == Some(&Trap::OutOfFuel) {
            Self::OutOfFuel
        } else {
            Self::Trap {
                reason: format!("{err:#}"),
            }
        }
    }
}

/// The engine configuration that makes execution deterministic and bounded.
///
/// The compiler is pinned to Cranelift because `NaN` canonicalization is one
/// of its settings, so a change of the default compiler cannot drop it. The
/// threads proposal is off because a guest that waits on a shared memory
/// blocks its thread where fuel cannot reach it, and nothing in the host
/// spawns the threads that would wake it.
fn config() -> Config {
    let mut config = Config::new();
    config
        .strategy(Strategy::Cranelift)
        .cranelift_nan_canonicalization(true)
        .relaxed_simd_deterministic(true)
        .consume_fuel(true)
        .wasm_threads(false);
    config
}

/// The engine's side of the boundary: compiles components under the
/// deterministic, fuel-metered configuration and instantiates them.
pub struct Host {
    engine: Engine,
    linker: Linker<()>,
}

impl Host {
    /// Builds a host whose engine canonicalizes `NaN`s, makes relaxed SIMD
    /// deterministic, and meters fuel, and whose linker defines the
    /// `supercom:engine/host` interface. Each host owns its own engine.
    ///
    /// # Errors
    ///
    /// Fails if the engine cannot be configured, with [`Error::Compile`].
    pub fn new() -> Result<Self, Error> {
        let engine = Engine::new(&config()).map_err(|err| Error::compile(&err))?;
        let mut linker = Linker::new(&engine);
        linker
            .instance(HOST_INTERFACE)
            .and_then(|mut host| host.func_wrap("log", |_, (_message,): (String,)| Ok(())))
            .map_err(|err| Error::compile(&err))?;
        Ok(Self { engine, linker })
    }

    /// Compiles `bytes` as a component and instantiates it, linking only the
    /// `supercom:engine/host` interface. The instance must export the whole
    /// `plugin` world, and it owns its own store, so loading the same bytes
    /// twice gives two independent plugins.
    ///
    /// Instantiation runs guest code under all the fuel there is; only calls
    /// into the returned [`Plugin`] are bounded.
    ///
    /// # Errors
    ///
    /// [`Error::NotAComponent`] if `bytes` is a core module,
    /// [`Error::Compile`] if it is not valid, [`Error::Instantiate`] if the
    /// component imports anything the host does not provide or lacks an
    /// export of the world.
    pub fn load(&self, bytes: &[u8]) -> Result<Plugin, Error> {
        let component = self.compile(bytes)?;
        let mut store = Store::new(&self.engine, ());
        store
            .set_fuel(INSTANTIATE_FUEL)
            .map_err(|err| Error::instantiate(&err))?;
        let instance = self
            .linker
            .instantiate(&mut store, &component)
            .map_err(|err| Error::instantiate(&err))?;
        let exports = Exports::new(&instance, &mut store)?;
        Ok(Plugin { store, exports })
    }

    /// Lists the fully qualified names of the interfaces a component imports,
    /// for example `supercom:engine/host@0.1.0`, in the order the component
    /// declares them, without instantiating it.
    ///
    /// # Errors
    ///
    /// The same as [`Host::load`] for bytes that are not a component.
    pub fn imports(&self, bytes: &[u8]) -> Result<Vec<String>, Error> {
        let component = self.compile(bytes)?;
        Ok(component
            .component_type()
            .imports(&self.engine)
            .map(|(name, _)| name.to_owned())
            .collect())
    }

    /// Compiles `bytes`, telling a core module from a component by its
    /// preamble alone rather than by the text of a compiler error. Only the
    /// binary format is read, never the text format.
    fn compile(&self, bytes: &[u8]) -> Result<Component, Error> {
        if bytes.starts_with(&CORE_MODULE_PREAMBLE) {
            return Err(Error::NotAComponent);
        }
        Component::from_binary(&self.engine, bytes).map_err(|err| Error::compile(&err))
    }
}

/// One instance of a plugin component, with its own state and fuel.
pub struct Plugin {
    store: Store<()>,
    exports: Exports,
}

/// The functions of the `plugin` world, looked up once when the plugin loads
/// so that a component that lacks one fails at load, not mid-match.
struct Exports {
    describe: TypedFunc<(), (String,)>,
    step: TypedFunc<(u64,), (u64,)>,
    nan_bits: TypedFunc<(), (u64,)>,
    spin: TypedFunc<(), ()>,
}

impl Exports {
    fn new(instance: &Instance, store: &mut Store<()>) -> Result<Self, Error> {
        Ok(Self {
            describe: export(instance, store, "describe")?,
            step: export(instance, store, "step")?,
            nan_bits: export(instance, store, "nan-bits")?,
            spin: export(instance, store, "spin")?,
        })
    }
}

/// Looks up the export `name` with the type the caller asks for, and names the
/// export in the error, which `wasmtime` does not.
fn export<Params, Results>(
    instance: &Instance,
    store: &mut Store<()>,
    name: &str,
) -> Result<TypedFunc<Params, Results>, Error>
where
    Params: ComponentNamedList + Lower,
    Results: ComponentNamedList + Lift,
{
    instance
        .get_typed_func(store, name)
        .map_err(|err| Error::Instantiate {
            reason: format!("export `{name}`: {err:#}"),
        })
}

impl Plugin {
    /// Calls the plugin's `describe` export.
    ///
    /// # Errors
    ///
    /// [`Error::OutOfFuel`] if the call exhausts its fuel, [`Error::Trap`] if
    /// the guest traps otherwise.
    pub fn describe(&mut self) -> Result<String, Error> {
        let describe = self.exports.describe;
        self.run(DESCRIBE_FUEL, |store| describe.call(store, ()))
            .map(|(text,)| text)
    }

    /// Calls the plugin's `step` export for `tick`, allowing the call `fuel`.
    ///
    /// # Errors
    ///
    /// As for [`Plugin::describe`].
    pub fn step(&mut self, tick: u64, fuel: u64) -> Result<u64, Error> {
        let step = self.exports.step;
        self.run(fuel, |store| step.call(store, (tick,)))
            .map(|(result,)| result)
    }

    /// Calls the plugin's `nan-bits` export, allowing the call `fuel`.
    ///
    /// # Errors
    ///
    /// As for [`Plugin::describe`].
    pub fn nan_bits(&mut self, fuel: u64) -> Result<u64, Error> {
        let nan_bits = self.exports.nan_bits;
        self.run(fuel, |store| nan_bits.call(store, ()))
            .map(|(bits,)| bits)
    }

    /// Calls the plugin's `spin` export, which never returns on its own,
    /// allowing the call `fuel`.
    ///
    /// # Errors
    ///
    /// [`Error::OutOfFuel`] once the fuel is used up, which is the only way
    /// the call ends.
    pub fn spin(&mut self, fuel: u64) -> Result<(), Error> {
        let spin = self.exports.spin;
        self.run(fuel, |store| spin.call(store, ()))
    }

    /// Gives the store exactly `fuel`, then makes the call. Every call into
    /// the guest goes through here, so none runs on fuel left over from an
    /// earlier one.
    fn run<Output>(
        &mut self,
        fuel: u64,
        call: impl FnOnce(&mut Store<()>) -> wasmtime::Result<Output>,
    ) -> Result<Output, Error> {
        self.store.set_fuel(fuel).map_err(|err| Error::call(&err))?;
        call(&mut self.store).map_err(|err| Error::call(&err))
    }
}
