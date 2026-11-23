//! WP-2622 ([cpp.line]/3, [cpp.concat]/3): a token is written in
//! scratch space when it was spelled in the preprocessor's paste buffer,
//! whatever presumed name a `#line` gives the file it appears in.
#![forbid(unsafe_code)]

use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_lex::{Interner, Std, TokKind};
use kiln_pp::{PpOptions, Preprocessor};
use kiln_source::SourceManager;

/// `#line 1 "<scratch space>"` in a real file: its own tokens are not
/// scratch space; the result of a `##` paste is.
#[test]
fn line_directive_naming_scratch_space() {
    let interner = Interner::new();
    let sm = SourceManager::new();
    let src = "#define CAT(a, b) a##b\n#line 1 \"<scratch space>\"\nint CAT(x, y) = z;\n";
    let main = sm.add_virtual("t.cpp", src.to_owned()).unwrap();
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let mut pp = Preprocessor::new(&sm, &engine, &interner, PpOptions::new(Std::Cxx26));
    pp.enter_main_file(main);
    let mut seen = Vec::new();
    loop {
        let t = pp.next();
        if t.kind == TokKind::Eof {
            break;
        }
        let at = sm.spelling_loc(t.span.lo);
        assert_eq!(sm.presumed_line_col(t.span.lo).unwrap().file_name, "<scratch space>");
        seen.push(sm.is_written_in_scratch_space(at));
    }
    drop(pp);
    assert!(sink.take().is_empty());
    // `int`, `xy` (pasted), `=`, `z`, `;`.
    assert_eq!(seen, [false, true, false, false, false]);
}
