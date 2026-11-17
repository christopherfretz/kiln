//! The in-process back-end hook (spec/driver.md "Back-end selection",
//! "In-process back end"; WP-1559).
//!
//! A program that links `kiln_driver` as a library and a back end beside
//! it (an out-of-tree wrapper binary: a `kiln++` with hod built in) calls
//! [`register`] once, before it runs the driver. Under
//! `-fkiln-backend=hod` (or `KILN_BACKEND=hod`) the driver then hands each
//! module the hod route would take to the registered back end, in the
//! same process and before the module is printed, in place of running
//! the external `hod` executable. **Nothing in kiln's own build
//! registers a back end**: the stock `kiln++` takes the external route
//! (or the LLVM one), and this module costs it one atomic load per run.
//!
//! The hook is dependency-free: it names only `kiln_llvm`'s module and
//! standard types. The back end parses the `llc` arguments itself and
//! never opens a file: the driver writes the object where `llc -o` would
//! have written it. Its panics are contained ([`crate::crash::contained`]):
//! a panic on the calling thread or on a thread it started is a failure
//! with the reason `internal-panic`, never a crash of the compiler.
use std::sync::OnceLock;

/// A back end's answer for one module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// The object is complete in the buffer: the driver writes it where
    /// `llc -o` would have, and neither `opt` nor `llc` runs.
    Done,
    /// The back end does not take this module, for the stable reason
    /// given (such as `linkage:available_externally`): the driver prints
    /// the module and takes the LLVM route, silently (`-v`: one line).
    Declined(String),
    /// The back end failed (a bug in it): the driver warns
    /// (`-Whod-fallback`) and takes the LLVM route, or stops with an error
    /// under `KILN_HOD_STRICT=1` / `-Werror=hod-fallback`.
    Failed(String),
}

/// An in-process back end.
pub trait InProcessBackend: Send + Sync {
    /// Compiles `module` to an object appended to `object`. `llc_args`
    /// is exactly the argument list the driver would have given `llc` for
    /// this job (`-O0`, `-relocation-model=`, `-filetype=obj`, ...,
    /// ending `-o <object> -`), for the back end to parse; the `-o` file
    /// is the driver's to write. Called on the compile thread, inside a
    /// contained-panic scope.
    fn compile(
        &self,
        module: &kiln_llvm::Module,
        llc_args: &[String],
        object: &mut Vec<u8>,
    ) -> Answer;
}

static BACKEND: OnceLock<Box<dyn InProcessBackend>> = OnceLock::new();

/// Registers the process's in-process back end: once, before the driver
/// runs (a later registration would race the driver's reads).
///
/// # Errors
///
/// A back end is already registered: `backend` is handed back.
pub fn register(backend: Box<dyn InProcessBackend>) -> Result<(), Box<dyn InProcessBackend>> {
    BACKEND.set(backend)
}

/// The registered back end, if any.
pub fn registered() -> Option<&'static dyn InProcessBackend> {
    BACKEND.get().map(|b| &**b)
}
