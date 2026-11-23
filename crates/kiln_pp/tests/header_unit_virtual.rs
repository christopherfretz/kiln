//! WP-2621 (spec/modules.md §11.6, [module.import]/5): a header unit
//! imported from an in-memory buffer is keyed by the buffer's name as
//! given, not through the host's file system, so neither a host file of
//! that name nor a header-unit module file built from one stands for it.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_lex::{Interner, Std};
use kiln_pp::{
    MemoryResolver, ModuleOptions, PpOptions, Preprocessor, PrintOptions, print_preprocessed,
};
use kiln_source::{SourceManager, WorkDir};

/// Preprocesses the in-memory `main.cpp` (`import "h.h"; int y = X;`)
/// with the in-memory `h.h` (`#define X 1`) in the working directory
/// `dir`, given a header-unit module file for `dir/h.h` (which does not
/// exist: reading it would be an error). The output and the diagnostics.
fn run(dir: &Path) -> (String, Vec<String>) {
    let interner = Interner::new();
    let sm = SourceManager::new();
    sm.set_work_dir(WorkDir::at(dir).unwrap());
    let main = sm.add_virtual("main.cpp", "import \"h.h\";\nint y = X;\n".to_owned()).unwrap();
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let mut o = PpOptions::new(Std::Cxx26);
    let host_key = format!("\"{}", dir.join("h.h").display());
    o.modules = Some(ModuleOptions {
        module_files: vec![(host_key, dir.join("h.pcm"))],
        ..ModuleOptions::default()
    });
    let mut pp = Preprocessor::new(&sm, &engine, &interner, o);
    pp.set_include_resolver(Box::new(MemoryResolver::new().with_header("h.h", "#define X 1\n")));
    pp.enter_main_file(main);
    let mut out = String::new();
    print_preprocessed(
        &mut pp,
        PrintOptions { line_markers: false, line_directives: false, ucn_as_utf8: false },
        &mut out,
    );
    drop(pp);
    let diags = sink.take().iter().map(|d| d.message.clone()).collect();
    (out, diags)
}

/// [module.import]/5: the header unit is the buffer's (`X` is 1), with
/// and without a host `h.h` (`X` 2) in the working directory, and the
/// module file given for the host path is not read.
#[test]
fn a_virtual_header_unit_is_not_the_host_file() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("kiln_pp-hu-virtual-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dir = std::fs::canonicalize(&dir).unwrap();
    let without = run(&dir);
    std::fs::write(dir.join("h.h"), "#define X 2\n").unwrap();
    let with = run(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    for (out, diags) in [without, with] {
        assert!(diags.is_empty(), "{diags:?}");
        assert!(out.contains("int y = 1;"), "{out}");
    }
}
