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
//!
//! A component is untrusted, so the host also holds it to limits that no
//! caller can change. Input over `COMPONENT_MAX_BYTES` (16 MiB) is refused
//! before it is compiled. Instantiation, which runs start functions, has a
//! fuel budget of its own, `INSTANTIATE_FUEL`. Each store caps a linear memory
//! at `MEMORY_LIMIT_BYTES` (64 MiB) and a table at `TABLE_ELEMENTS` elements,
//! and allows `MAX_INSTANCES` instances, `MAX_MEMORIES` memories and
//! `MAX_TABLES` tables. Guest code may use `MAX_WASM_STACK_BYTES` (256 KiB) of
//! native stack, and one crossing of the boundary lifts at most
//! `HOSTCALL_FUEL` (64 KiB) of data from the guest.

use std::fmt;

use wasmtime::component::{
    Component, ComponentNamedList, Instance, Lift, Linker, Lower, TypedFunc, WasmStr,
};
use wasmtime::{
    Config, Engine, Store, StoreLimits, StoreLimitsBuilder, Strategy, Trap, WasmBacktraceDetails,
};

/// The one interface the host provides to a plugin.
const HOST_INTERFACE: &str = "supercom:engine/host@0.1.0";

/// How a core WebAssembly module starts: the magic `\0asm` and binary-format
/// version 1. A component starts with the same magic and the words `0d 00` and
/// `01 00` (component-model version and layer), so these eight bytes tell the
/// two apart before any compiling.
const CORE_MODULE_PREAMBLE: [u8; 8] = [0, 0x61, 0x73, 0x6d, 1, 0, 0, 0];

/// Fuel for instantiation, which runs start functions that would otherwise stall `Host::load`.
const INSTANTIATE_FUEL: u64 = 10_000_000;

/// The fuel [`Plugin::describe`] runs under, since it takes no budget.
const DESCRIBE_FUEL: u64 = 1_000_000;

/// The most a linear memory may reach, so a guest cannot exhaust the host's memory by growing.
const MEMORY_LIMIT_BYTES: usize = 64 * 1024 * 1024;

/// The most elements a table may hold, so a guest cannot exhaust the host's memory with a table.
const TABLE_ELEMENTS: usize = 10_000;

/// The most instances one store may create, so a component cannot instantiate without end.
const MAX_INSTANCES: usize = 16;

/// The most linear memories one store may create, each a reservation of address space.
const MAX_MEMORIES: usize = 4;

/// The most tables one store may create, each a further allocation.
const MAX_TABLES: usize = 4;

/// The native stack guest code may use, so unbounded recursion traps instead of overrunning a thread.
const MAX_WASM_STACK_BYTES: usize = 256 * 1024;

/// The bytes one crossing may lift from the guest, so a huge string cannot make the host copy it.
const HOSTCALL_FUEL: usize = 64 * 1024;

/// The largest input the host will compile, so the compiler is never fed an unbounded component.
const COMPONENT_MAX_BYTES: usize = 16 * 1024 * 1024;

/// Why a component could not be loaded or run.
#[derive(Debug)]
pub enum Error {
    /// The bytes do not compile as a component under the host's
    /// configuration, are longer than the host will compile, or the host's
    /// compiler could not be set up.
    Compile {
        /// What the compiler reported.
        reason: String,
    },
    /// The component compiled but could not be instantiated, for example
    /// because it imports an interface the host does not provide or asks for
    /// more memory, tables or instances than a store allows.
    Instantiate {
        /// What the linker or the instantiation reported.
        reason: String,
    },
    /// The guest trapped for a reason other than running out of fuel.
    Trap {
        /// What the trap reported.
        reason: String,
    },
    /// The guest used all the fuel it was given, in a call or while it was
    /// being instantiated, which is how the host stops a guest that does not
    /// return. Exhaustion is deterministic: the same component and the same
    /// fuel always stop at the same point.
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

    /// A linker or instantiation failure. A start function that ran out of
    /// fuel is [`Error::OutOfFuel`], as it is in a call.
    fn instantiate(err: &wasmtime::Error) -> Self {
        if out_of_fuel(err) {
            Self::OutOfFuel
        } else {
            Self::Instantiate {
                reason: format!("{err:#}"),
            }
        }
    }

    /// What a failed guest call reported: exhausted fuel is its own variant
    /// and every other trap is a [`Error::Trap`].
    fn call(err: &wasmtime::Error) -> Self {
        if out_of_fuel(err) {
            Self::OutOfFuel
        } else {
            Self::Trap {
                reason: format!("{err:#}"),
            }
        }
    }
}

/// Whether `err` is the trap that metered fuel raises when it runs out.
fn out_of_fuel(err: &wasmtime::Error) -> bool {
    err.downcast_ref::<Trap>() == Some(&Trap::OutOfFuel)
}

/// What every store holds besides the guest: the limits its limiter enforces.
struct State {
    limits: StoreLimits,
}

impl State {
    fn new() -> Self {
        Self {
            limits: StoreLimitsBuilder::new()
                .memory_size(MEMORY_LIMIT_BYTES)
                .table_elements(TABLE_ELEMENTS)
                .instances(MAX_INSTANCES)
                .memories(MAX_MEMORIES)
                .tables(MAX_TABLES)
                .build(),
        }
    }
}

/// The engine configuration that makes execution deterministic and bounded.
///
/// The compiler is pinned to Cranelift because `NaN` canonicalization is one
/// of its settings, so a change of the default compiler cannot drop it. The
/// threads proposal is off because a guest that waits on a shared memory
/// blocks its thread where fuel cannot reach it, and nothing in the host
/// spawns the threads that would wake it. The native stack guest code may use
/// is pinned rather than left to the default, so that the amount a caller must
/// have free does not change with the `wasmtime` version.
///
/// Four proposals the plugin world has no use for are off as well: 64-bit
/// memories, garbage collection, exception handling (whose objects live on
/// the garbage-collected heap) and the component model's async ABI (which
/// schedules guest tasks beyond the fuel of any one call). Seven others stay
/// at `wasmtime` 49's defaults, which turn them on:
/// tail calls,
/// function references,
/// wide arithmetic,
/// SIMD,
/// relaxed SIMD (made deterministic above),
/// multi-memory,
/// and extended-const.
/// A `wasmtime` upgrade that changes one of those defaults widens what a
/// guest may use without a diff here, so the plugin-api specification pins
/// the set it admits. Backtrace details are off so that the text of a trap
/// does not depend on an environment variable or on debug information the
/// guest supplies.
fn config() -> Config {
    let mut config = Config::new();
    config
        .strategy(Strategy::Cranelift)
        .cranelift_nan_canonicalization(true)
        .relaxed_simd_deterministic(true)
        .consume_fuel(true)
        .max_wasm_stack(MAX_WASM_STACK_BYTES)
        .wasm_threads(false)
        .wasm_memory64(false)
        .wasm_gc(false)
        .wasm_exceptions(false)
        .concurrency_support(false)
        .wasm_backtrace_details(WasmBacktraceDetails::Disable);
    config
}

/// The engine's side of the boundary: compiles components under the
/// deterministic, fuel-metered configuration and instantiates them.
pub struct Host {
    engine: Engine,
    linker: Linker<State>,
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
        // The message is taken as a `WasmStr` and never read: the host checks
        // that it lies inside the guest's memory and charges it against the
        // hostcall fuel, but does not copy or decode it.
        linker
            .instance(HOST_INTERFACE)
            .and_then(|mut host| host.func_wrap("log", |_, (_message,): (WasmStr,)| Ok(())))
            .map_err(|err| Error::compile(&err))?;
        Ok(Self { engine, linker })
    }

    /// Compiles `bytes` as a component and instantiates it, linking only the
    /// `supercom:engine/host` interface. The instance must export the whole
    /// `plugin` world, and it owns its own store, so loading the same bytes
    /// twice gives two independent plugins.
    ///
    /// Instantiation runs guest code, so it has a fuel budget of its own;
    /// calls into the returned [`Plugin`] are metered separately.
    ///
    /// # Errors
    ///
    /// [`Error::NotAComponent`] if `bytes` is a core module,
    /// [`Error::Compile`] if it is not valid or is longer than 16 MiB,
    /// [`Error::OutOfFuel`] if a start function does not return within the
    /// instantiation budget, [`Error::Instantiate`] if the component imports
    /// anything the host does not provide, lacks an export of the world, or
    /// asks for more memory, tables or instances than a store allows.
    pub fn load(&self, bytes: &[u8]) -> Result<Plugin, Error> {
        let component = self.compile(bytes)?;
        let mut store = self.store();
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

    /// A new store with every limit in place. Every store comes from here, so
    /// none runs a guest under the defaults.
    fn store(&self) -> Store<State> {
        let mut store = Store::new(&self.engine, State::new());
        store.set_hostcall_fuel(HOSTCALL_FUEL);
        store.limiter(|state| &mut state.limits);
        store
    }

    /// Compiles `bytes`, telling a core module from a component by its
    /// preamble alone rather than by the text of a compiler error. Only the
    /// binary format is read, never the text format. Input over the size limit
    /// is refused first, so the work that follows is bounded.
    fn compile(&self, bytes: &[u8]) -> Result<Component, Error> {
        if bytes.len() > COMPONENT_MAX_BYTES {
            return Err(Error::Compile {
                reason: format!(
                    "the input is {} bytes, over the limit of {COMPONENT_MAX_BYTES}",
                    bytes.len()
                ),
            });
        }
        if bytes.starts_with(&CORE_MODULE_PREAMBLE) {
            return Err(Error::NotAComponent);
        }
        Component::from_binary(&self.engine, bytes).map_err(|err| Error::compile(&err))
    }
}

/// One instance of a plugin component, with its own state and fuel.
///
/// A plugin fails closed. When any call into it returns an error, whether the
/// guest ran out of fuel, trapped, overran its stack, or tried to pass more
/// data than the host lifts in one crossing, the plugin is retired: every
/// later call returns [`Error::Trap`] without running guest code, because
/// what the guest left behind cannot be trusted. Load the component again for
/// a fresh instance.
///
/// Guest code runs on the stack of the thread that calls it, and may use up to
/// `MAX_WASM_STACK_BYTES` (256 KiB) of it before it traps. A caller therefore
/// needs at least that much free stack plus the host's own frames: on a
/// thread with less, a guest that recurses without bound overruns the thread,
/// and that aborts the process.
pub struct Plugin {
    store: Store<State>,
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
    fn new(instance: &Instance, store: &mut Store<State>) -> Result<Self, Error> {
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
    store: &mut Store<State>,
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
    /// Calls the plugin's `describe` export. The text it answers may be at
    /// most 64 KiB, the most the host lifts from the guest in one crossing.
    ///
    /// # Errors
    ///
    /// [`Error::OutOfFuel`] if the call exhausts its fuel, [`Error::Trap`] if
    /// the guest traps otherwise or answers with more text than the host lifts.
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
    /// the call ends, and which retires the plugin.
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
        call: impl FnOnce(&mut Store<State>) -> wasmtime::Result<Output>,
    ) -> Result<Output, Error> {
        self.store.set_fuel(fuel).map_err(|err| Error::call(&err))?;
        call(&mut self.store).map_err(|err| Error::call(&err))
    }
}

#[cfg(test)]
mod tests;
