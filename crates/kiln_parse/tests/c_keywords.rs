//! WP-1275: the parser's C keyword set ([`Keywords::for_c`], spec/parse.md
//! §1; C89 3.1.1, C99/C11/C17/C23 6.4.1) agrees with clang 19's
//! `__is_identifier` in C, which the preprocessor answers from the
//! generated table (`ClangFeatures::with_c`, itself checked against
//! `clang-19 -x c` in kiln_pp's features_c test).

use kiln_lex::{CStd, Interner, Std};
use kiln_parse::keywords::{C23_KEYWORDS, CXX_ONLY_KEYWORDS, CXX20_KEYWORDS, Keywords};
use kiln_pp::{ClangCompat, ClangFeatures, FeatureQueries};

const STDS: [(CStd, Std); 5] = [
    (CStd::C89, Std::Cxx98),
    (CStd::C99, Std::Cxx98),
    (CStd::C11, Std::Cxx11),
    (CStd::C17, Std::Cxx11),
    (CStd::C23, Std::Cxx17),
];

/// The table the parser builds for C standard `c` (parser.rs).
fn table(interner: &Interner, c: CStd, lex: Std, gnu: bool) -> Keywords {
    let mut kw = Keywords::with_std(interner, gnu, lex >= Std::Cxx11, false, 19);
    for k in CXX20_KEYWORDS {
        kw.remove(interner, k);
    }
    kw.for_c(interner, c, gnu);
    kw
}

#[test]
fn c_keywords_match_clang() {
    let mut names: Vec<&str> =
        CXX_ONLY_KEYWORDS.iter().chain(C23_KEYWORDS.iter()).copied().collect();
    names.extend([
        "_Bool",
        "restrict",
        "inline",
        "asm",
        "typeof",
        "typeof_unqual",
        "int",
        "struct",
        "auto",
        "register",
        "_Alignas",
        "_Atomic",
        "_Generic",
        "_Noreturn",
        "_Static_assert",
        "_Thread_local",
        "__typeof__",
        "__restrict",
        "__inline__",
        "__asm__",
    ]);
    let mut bad = Vec::new();
    for (c, lex) in STDS {
        for gnu in [false, true] {
            let interner = Interner::new();
            let kw = table(&interner, c, lex, gnu);
            let args: &[&str] = if gnu { &["-std=gnu++"] } else { &[] };
            let f = ClangFeatures::from_args("x86_64-unknown-linux-gnu", lex, args)
                .unwrap()
                .with_compat(ClangCompat::Clang19)
                .with_c(c, gnu);
            for n in &names {
                let parser = kw.get(interner.intern(n)).is_some();
                if parser != f.is_keyword(n) {
                    bad.push(format!("{c:?} gnu={gnu} {n}: parser {parser}, clang {}", !parser));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
