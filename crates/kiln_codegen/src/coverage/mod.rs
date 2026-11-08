//! Coverage instrumentation (WP-941, spec/codegen.md "Instrumentation"):
//! gcov's `!llvm.gcov` files (the counters themselves are `opt`'s
//! `insert-gcov-profiling`), and clang's source-based coverage: the
//! `llvm.instrprof.increment` counters of `CodeGenPGO` and the
//! `__llvm_covmap` / `__llvm_covfun` mapping of `CoverageMappingGen`.

/// The coverage options of a compilation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CoverageOptions {
    /// gcov (`--coverage`, `-fprofile-arcs`): the `(notes, data)` files
    /// `!llvm.gcov` names (clang `-coverage-notes-file=` /
    /// `-coverage-data-file=`). Needs debug locations: the driver turns
    /// [`crate::DebugInfoKind::None`] into `LocTrackingOnly`.
    pub gcov: Option<(String, String)>,
    /// `-fprofile-instr-generate`: InstrProf counters.
    pub instr: bool,
    /// `-fprofile-instr-generate=<path>`: the runtime's output file
    /// (`@__llvm_profile_filename`).
    pub instr_path: Option<String>,
    /// `-fcoverage-mapping`: the coverage mapping records.
    pub mapping: bool,
    /// The coverage compilation directory (the first filename of the
    /// mapping; clang `-fcoverage-compilation-dir=`, the working directory
    /// by default).
    pub comp_dir: String,
    /// `-fcoverage-prefix-map=` / `-ffile-prefix-map=` pairs, in command
    /// line order.
    pub prefix_map: Vec<(String, String)>,
}
