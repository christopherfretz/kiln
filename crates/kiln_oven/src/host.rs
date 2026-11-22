//! Host calls (spec/host-calls.md, WP-1990): the Rust interface a tooling
//! client implements so that constant evaluation can call into it
//! through `__builtin_kiln_host_call`. The call itself runs in
//! `vm/host.rs`.

use crate::Oven;

/// A host-call handler: each evaluated `__builtin_kiln_host_call(op, in,
/// n_in, bin, n_bin, out, cap_out, bout, cap_bout)` calls
/// [`HostCalls::call`] once (spec/host-calls.md §2). The trait has no
/// bound (WP-2321): a *registered* handler is a `Box<dyn HostCalls +
/// Send>` ([`Oven::set_host_calls`]), so that an Oven holding one can
/// still move to another thread (an Oven is used by one thread at a time;
/// nothing needs `Sync`), while an evaluation-session run's handler is
/// only borrowed for the call (`&mut dyn HostCalls`) and may hold an `Rc`
/// or borrow another unit's AST.
pub trait HostCalls {
    /// Runs operation `op` on the integer arguments `ints` (the `n_in`
    /// elements at `in`) and the byte arguments `bytes` (the `n_bin` at
    /// `bin`), writing its results to `out`. The `Ok` value is the
    /// builtin's result, which kiln does not interpret. `Err` ends the
    /// evaluation with the error's message (it cannot be caught).
    ///
    /// # Errors
    /// Whatever the handler decides: the evaluation then fails.
    fn call(
        &mut self,
        op: u32,
        ints: &[u64],
        bytes: &[u8],
        out: &mut HostOut<'_>,
    ) -> Result<u64, HostError>;
}

/// A host call's failure: its message is the diagnostic's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostError {
    msg: Box<str>,
}

impl HostError {
    /// The failure with `msg`.
    pub fn new(msg: impl Into<String>) -> HostError {
        HostError { msg: msg.into().into_boxed_str() }
    }

    /// The message.
    pub fn message(&self) -> &str {
        &self.msg
    }
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

impl std::error::Error for HostError {}

/// The results of one host call: up to [`HostOut::int_capacity`]
/// integers (stored at `out`) and [`HostOut::byte_capacity`] bytes
/// (stored at `bout`). Exactly the elements written are stored; writing
/// past a capacity is kept and fails the evaluation after the call.
#[derive(Debug)]
pub struct HostOut<'a> {
    ints: &'a mut Vec<u64>,
    bytes: &'a mut Vec<u8>,
    cap_ints: usize,
    cap_bytes: usize,
}

impl<'a> HostOut<'a> {
    /// Results into `ints` and `bytes` (cleared here) with the given
    /// capacities: how the Oven calls a handler, and how a handler's own
    /// tests can.
    pub fn new(
        ints: &'a mut Vec<u64>,
        bytes: &'a mut Vec<u8>,
        cap_ints: usize,
        cap_bytes: usize,
    ) -> HostOut<'a> {
        ints.clear();
        bytes.clear();
        HostOut { ints, bytes, cap_ints, cap_bytes }
    }

    /// `cap_out`: how many integers the call may write.
    pub fn int_capacity(&self) -> usize {
        self.cap_ints
    }

    /// `cap_bout`: how many bytes the call may write.
    pub fn byte_capacity(&self) -> usize {
        self.cap_bytes
    }

    /// Appends `v` to the integer results.
    pub fn write_ints(&mut self, v: &[u64]) {
        self.ints.extend_from_slice(v);
    }

    /// Appends `v` to the byte results.
    pub fn write_bytes(&mut self, v: &[u8]) {
        self.bytes.extend_from_slice(v);
    }

    /// The integers written so far.
    pub fn ints_written(&self) -> usize {
        self.ints.len()
    }

    /// The bytes written so far.
    pub fn bytes_written(&self) -> usize {
        self.bytes.len()
    }
}

/// Where the running evaluation stands for host calls
/// (spec/host-calls.md §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HostMode {
    /// A call runs (unless the VM is in a trial: then as `Speculative`).
    Allowed,
    /// A call fails the evaluation without a note.
    Silent,
    /// A call is the error "host call during speculative evaluation".
    Speculative,
}

/// The Oven's host-call state.
pub(crate) struct Host {
    pub(crate) handler: Option<Box<dyn HostCalls + Send>>,
    /// The running evaluation's mode; `Silent` at rest.
    pub(crate) mode: HostMode,
    /// Calls the handler received.
    pub(crate) calls: u64,
    /// The arguments read and the results written, reused across calls.
    pub(crate) ints_in: Vec<u64>,
    pub(crate) bytes_in: Vec<u8>,
    pub(crate) ints_out: Vec<u64>,
    pub(crate) bytes_out: Vec<u8>,
}

impl Default for Host {
    fn default() -> Host {
        Host {
            handler: None,
            mode: HostMode::Silent,
            calls: 0,
            ints_in: Vec::new(),
            bytes_in: Vec::new(),
            ints_out: Vec::new(),
            bytes_out: Vec::new(),
        }
    }
}

impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Host")
            .field("handler", &self.handler.is_some())
            .field("mode", &self.mode)
            .field("calls", &self.calls)
            .finish_non_exhaustive()
    }
}

impl Oven {
    /// Registers `handler` for `__builtin_kiln_host_call` (`None`: none),
    /// returning the previous one (spec/host-calls.md §3). The name is
    /// declared by sema (`kiln_sema::Sema::set_host_calls`), not here.
    pub fn set_host_calls(
        &mut self,
        handler: Option<Box<dyn HostCalls + Send>>,
    ) -> Option<Box<dyn HostCalls + Send>> {
        std::mem::replace(&mut self.host.handler, handler)
    }

    /// Removes and returns the registered handler.
    pub fn take_host_calls(&mut self) -> Option<Box<dyn HostCalls + Send>> {
        self.host.handler.take()
    }

    /// Whether a host-call handler is registered.
    pub fn has_host_calls(&self) -> bool {
        self.host.handler.is_some()
    }

    /// How many calls the handler received.
    pub fn host_call_count(&self) -> u64 {
        self.host.calls
    }

    /// Enters an entry point of host mode `m` (spec/host-calls.md §4):
    /// an outermost evaluation takes `m`; a nested one keeps a silent
    /// evaluation's mode, an allowed one's unless `m` refuses, and a
    /// speculative one's unless `m` is silent. Returns the mode to
    /// restore ([`Oven::host_leave`]).
    pub(crate) fn host_enter(&mut self, m: HostMode) -> HostMode {
        let saved = self.host.mode;
        self.host.mode = match (m, saved) {
            _ if self.ctx_depth == 0 => m,
            (HostMode::Allowed, s) => s,
            (HostMode::Silent, _) | (HostMode::Speculative, HostMode::Silent) => HostMode::Silent,
            (HostMode::Speculative, _) => HostMode::Speculative,
        };
        saved
    }

    /// Leaves an entry point: restores the mode [`Oven::host_enter`]
    /// returned.
    pub(crate) fn host_leave(&mut self, saved: HostMode) {
        self.host.mode = saved;
    }
}
