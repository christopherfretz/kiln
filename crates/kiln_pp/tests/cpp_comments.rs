//! The comment handler (clang `CommentHandler`, spec/pp.md "Comment
//! handler"): every `//` and `/* */` comment the lexer produces, in the
//! main file and in included files, in source order; none from skipped
//! groups or from the raw text of `#warning`. Used by `-verify`
//! (spec/diag.md §11.1).

mod common;

use std::cell::RefCell;
use std::rc::Rc;

use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_lex::{Interner, Std};
use kiln_pp::{MemoryResolver, PpOptions, Preprocessor, PrintOptions, print_preprocessed};
use kiln_source::SourceManager;

/// [lex.comment]/1, [cpp.cond]/13, [cpp.include]: comments handed to the
/// handler as spans over their full text.
#[test]
fn comment_handler_sees_lexed_comments() {
    let interner = Interner::new();
    let sm = SourceManager::new();
    let src = "// one\n#include \"a.h\" /* two */\n#if 0\n// skipped\n#endif\n#warning w // raw\nint /* three */ x; // four";
    let main = sm.add_virtual("t.cpp", src.to_owned()).unwrap();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let mut opts = PpOptions::new(Std::Cxx26);
    common::PREDEFINES.clone_into(&mut opts.predefines);
    let seen = Rc::new(RefCell::new(Vec::new()));
    {
        let mut pp = Preprocessor::new(&sm, &engine, &interner, opts);
        pp.set_include_resolver(Box::new(
            MemoryResolver::new().with_header("a.h", "/* in a */ int a;\n"),
        ));
        pp.enter_main_file(main);
        let (seen2, sm2) = (seen.clone(), &sm);
        pp.set_comment_handler(Box::new(move |s| {
            seen2.borrow_mut().push(sm2.span_text(s).unwrap_or("?").to_owned());
        }));
        let mut text = String::new();
        print_preprocessed(&mut pp, PrintOptions::default(), &mut text);
    }
    assert_eq!(*seen.borrow(), ["// one", "/* two */", "/* in a */", "/* three */", "// four"]);
}
