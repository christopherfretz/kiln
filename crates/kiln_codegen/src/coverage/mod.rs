//! Coverage instrumentation (WP-941, spec/codegen.md "Instrumentation"):
//! gcov's `!llvm.gcov` files (the counters themselves are `opt`'s
//! `insert-gcov-profiling`), and clang's source-based coverage: the
//! `llvm.instrprof.increment` counters of `CodeGenPGO` and the
//! `__llvm_covmap` / `__llvm_covfun` mapping of `CoverageMappingGen`
//! (WP-1003, [`mapping`]).

pub(crate) mod encode;
pub(crate) mod mapping;
pub(crate) mod pgo;
pub(crate) mod records;
pub(crate) mod skipped;

/// The coverage options of a compilation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CoverageOptions {
    /// gcov (`--coverage`, `-fprofile-arcs`): the `(notes, data)` files
    /// `!llvm.gcov` names (clang `-coverage-notes-file=` /
    /// `-coverage-data-file=`). Needs debug locations: the driver turns
    /// [`crate::DebugInfoKind::None`] into `LocTrackingOnly`.
    pub gcov: Option<(String, String)>,
    /// `-fprofile-instr-generate`: `InstrProf` counters.
    pub instr: bool,
    /// `-fprofile-instr-generate=<path>`: the runtime's output file
    /// (`@__llvm_profile_filename`, which clang's `InstrProfiling` lowering
    /// creates; kiln emits it with the module, and not under
    /// `-disable-llvm-passes`).
    pub instr_path: Option<String>,
    /// `-fcoverage-mapping`: the coverage mapping records (with
    /// [`Self::instr`]).
    pub mapping: bool,
    /// The groups the preprocessor skipped (`#if 0` ...), for the
    /// mapping's skipped regions: each from the `#` of the directive that
    /// started skipping to the end of the line of the one that ended it
    /// (`kiln_pp::PpCallbacks::source_range_skipped`).
    pub skipped: Vec<kiln_source::Span>,
    /// The macro invocations written in files (`PpCallbacks::macro_expands`
    /// ranges with file locations), with whether the macro is
    /// function-like, for the skipped regions' token model.
    pub expanded: Vec<(kiln_source::Span, bool)>,
    /// The coverage compilation directory (the first filename of the
    /// mapping; clang `-fcoverage-compilation-dir=`, the working directory
    /// by default).
    pub comp_dir: String,
    /// `-fcoverage-prefix-map=` / `-ffile-prefix-map=` pairs, in command
    /// line order.
    pub prefix_map: Vec<(String, String)>,
}
