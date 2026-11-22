//! The preprocessing record (spec/pp.md "Preprocessing record"): the
//! parameter-use classifier against real expansions, one definition
//! expanded many times, stability, `-E` identity with recording on and
//! off, a header's keys under two main files, and the gap flags
//! ([cpp.replace], [cpp.subst], [cpp.stringize], [cpp.concat],
//! [cpp.rescan]).

mod common;

use common::{PREDEFINES, run_with};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_lex::{Interner, Std, TokKind};
use kiln_pp::state::PpState;
use kiln_pp::{
    DefSource, ExpIx, MadeKind, MemoryResolver, Origin, ParamUse, PpOptions, PpRecord,
    Preprocessor, PrintOptions,
};
use kiln_source::{Loc, SourceManager};
use std::collections::HashSet;
use std::fmt::Write as _;

/// Runs `src` (main file `t.cpp`, headers from `r`) with recording on,
/// reads every token, and calls `f` with the record and the tokens'
/// `(location, spelling)`.
fn with_record<R>(
    src: &str,
    r: MemoryResolver,
    f: impl FnOnce(&SourceManager, &Interner, &PpRecord, &[(Loc, String)]) -> R,
) -> R {
    let interner = Interner::new();
    let sm = SourceManager::new();
    let main = sm.add_virtual("t.cpp", src.to_owned()).unwrap();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let mut opts = PpOptions::new(Std::Cxx26);
    PREDEFINES.clone_into(&mut opts.predefines);
    opts.reproducible_time = Some(("Jan  1 2000".to_owned(), "00:00:00".to_owned()));
    opts.pp_record = true;
    let mut pp = Preprocessor::new(&sm, &engine, &interner, opts);
    pp.set_include_resolver(Box::new(r));
    pp.enter_main_file(main);
    let mut toks = Vec::new();
    loop {
        let t = pp.next();
        if t.kind == TokKind::Eof {
            break;
        }
        toks.push((t.span.lo, kiln_lex::spelling(&sm, &t, pp.lex_options()).into_owned()));
    }
    let rec = pp.record().expect("recording is on");
    f(&sm, &interner, rec, &toks)
}

/// The dump of `src` with headers from `r`.
fn dump(src: &str, r: MemoryResolver) -> String {
    with_record(src, r, |sm, interner, rec, toks| rec.dump(sm, interner, toks))
}

/// Macros using every kind of parameter use, each expanded with
/// non-empty arguments so that every use produces a token.
const USES: &str = "\
#define CAT(a, b) a ## b
#define STR(x) #x
#define XSTR(x) STR(x)
#define ID(x) x
#define MIX(a, b) a #b a ## b b
#define VA(f, ...) f(__VA_ARGS__) __VA_OPT__(, __VA_ARGS__)
#define VS(...) #__VA_OPT__(__VA_ARGS__)
#define VP(a, ...) a ## __VA_OPT__(__VA_ARGS__)
#define TAIL(a, ...) __VA_ARGS__ ## a
#define G(n) (n)
#define CALL(f, v) f(v) + ID(v)
int CAT(ab, cd);
const char *s = STR(1 + 2);
const char *t = XSTR(ID(3));
MIX(p q, r s)
VA(g, 1, 2)
VS(a b)
VP(x, y z)
TAIL(k, m n)
CALL(G, ID(4))
";

/// Every origin the record gives agrees with the classifier: a
/// substituted argument token sits at a `Plain` or `PasteOperand` use of
/// its parameter, a pasted token's operands are `PasteOperand` uses, a
/// stringized token ends at a `Stringized` use, and every use the
/// classifier lists is seen in some expansion.
#[test]
fn param_uses_match_expansions() {
    with_record(USES, MemoryResolver::new(), |sm, _, rec, toks| {
        assert!(rec.is_complete());
        // Every macro location: the output tokens, the expansions' names
        // (a macro name substituted from an argument is consumed), the
        // arguments' tokens, and the locations before substitution,
        // transitively.
        let mut work: Vec<Loc> = toks.iter().map(|t| t.0).collect();
        for e in all_exps(rec) {
            work.push(rec.exp(e).name);
            for a in rec.args(e) {
                work.extend_from_slice(a.toks);
            }
        }
        let mut seen_locs = HashSet::new();
        // (definition index, body token) of every use observed.
        let mut observed = HashSet::new();
        while let Some(loc) = work.pop() {
            if !sm.is_macro_loc(loc) || !seen_locs.insert(loc) {
                continue;
            }
            match rec.origin(sm, loc) {
                Origin::Arg { exp, arg, use_tok, from, index } => {
                    let d = rec.exp(exp).def;
                    let uses = rec.param_uses(d, arg as usize);
                    let u = uses.iter().find(|u| u.tok == use_tok).unwrap_or_else(|| {
                        panic!("argument {arg} substituted at b{use_tok}, not a use: {uses:?}")
                    });
                    assert_ne!(u.kind, ParamUse::Stringized, "substituted at a `#` operand");
                    if let Some(i) = index {
                        let a = rec.args(exp).nth(arg as usize).unwrap();
                        assert_eq!(a.toks[i as usize], from, "index names `from`");
                    }
                    observed.insert((d.index(), use_tok));
                    work.push(from);
                }
                Origin::Made { exp: Some(exp), kind, body: Some((i, j)), .. } => {
                    let d = rec.exp(exp).def;
                    let def = &rec.def(d).def;
                    for p in 0..def.params().len() {
                        for u in rec.param_uses(d, p) {
                            if u.tok < i || u.tok > j {
                                continue;
                            }
                            match kind {
                                MadeKind::Paste if !u.in_va_opt => {
                                    assert_eq!(u.kind, ParamUse::PasteOperand, "paste b{i}..b{j}");
                                }
                                MadeKind::Stringize => {
                                    assert_eq!(u.kind, ParamUse::Stringized, "str b{i}..b{j}");
                                }
                                _ => {}
                            }
                            observed.insert((d.index(), u.tok));
                        }
                    }
                }
                Origin::Unknown => panic!("unknown origin for a location made while recording"),
                _ => {}
            }
        }
        let mut missing = Vec::new();
        for (di, d) in rec.defs().iter().enumerate() {
            if d.source != DefSource::File {
                continue;
            }
            for p in 0..d.def.params().len() {
                let ix = rec.def_of(&d.def).unwrap();
                for u in rec.param_uses(ix, p) {
                    if !observed.contains(&(di, u.tok)) {
                        missing.push(format!("def {di} param {p} {u:?}"));
                    }
                }
            }
        }
        assert!(missing.is_empty(), "uses never observed: {missing:#?}");
    });
}

/// The classifier's kinds for each macro of `USES`, by name.
#[test]
fn param_use_kinds() {
    with_record(USES, MemoryResolver::new(), |_, interner, rec, _| {
        let kinds = |name: &str| -> Vec<String> {
            let d = rec.defs().iter().position(|d| interner.as_str(d.def.name()) == name).unwrap();
            let ix = rec.def_of(&rec.defs()[d].def).unwrap();
            (0..rec.defs()[d].def.params().len())
                .flat_map(|p| rec.param_uses(ix, p))
                .map(|u| format!("{}{:?}{}", u.tok, u.kind, if u.in_va_opt { "/va" } else { "" }))
                .collect()
        };
        assert_eq!(kinds("CAT"), ["0PasteOperand", "2PasteOperand"]);
        assert_eq!(kinds("STR"), ["1Stringized"]);
        assert_eq!(kinds("XSTR"), ["2Plain"]);
        assert_eq!(
            kinds("MIX"),
            ["0Plain", "3PasteOperand", "2Stringized", "5PasteOperand", "6Plain"]
        );
        assert_eq!(kinds("VA"), ["0Plain", "2Plain", "7Plain/va"]);
        assert_eq!(kinds("VS"), ["3Stringized/va"]);
        assert_eq!(kinds("VP"), ["0PasteOperand", "4Plain/va"]);
        assert_eq!(kinds("TAIL"), ["2PasteOperand", "0PasteOperand"]);
    });
}

/// Every expansion, through the definitions'.
fn all_exps(rec: &PpRecord) -> Vec<ExpIx> {
    let mut v: Vec<ExpIx> = rec
        .defs()
        .iter()
        .filter_map(|d| rec.def_of(&d.def))
        .flat_map(|d| rec.expansions_of(d).collect::<Vec<_>>())
        .collect();
    v.sort_unstable_by_key(|e| e.index());
    v.dedup();
    assert_eq!(v.len(), rec.exps().len());
    v
}

/// One definition expanded 500 times: one definition, 500 expansions of
/// it, and each `L` is body token 0 of its own expansion.
#[test]
fn one_definition_many_expansions() {
    let mut src = String::from("#define LOG(m) L::log(m)\n");
    for i in 0..500 {
        let _ = writeln!(src, "LOG({i});");
    }
    with_record(&src, MemoryResolver::new(), |sm, _, rec, toks| {
        let files: Vec<_> = rec.defs().iter().filter(|d| d.source == DefSource::File).collect();
        assert_eq!(files.len(), 1);
        let d = rec.def_of(&files[0].def).unwrap();
        assert_eq!(rec.exps().len(), 500);
        assert!(rec.exps().iter().all(|e| e.def == d));
        assert_eq!(rec.expansions_of(d).count(), 500);
        let mut exps = HashSet::new();
        for (loc, sp) in toks {
            if sp == "L" {
                let Origin::Body { exp, tok: 0 } = rec.origin(sm, *loc) else {
                    panic!("`L` is not body token 0: {:?}", rec.origin(sm, *loc));
                };
                exps.insert(exp);
            }
        }
        assert_eq!(exps.len(), 500);
        assert!(rec.bytes() > 0);
    });
}

/// The same input gives the same record (with `__DATE__`, `__TIME__` and
/// `__COUNTER__` among the expansions).
#[test]
fn stable() {
    let src = "#include \"h.h\"\n#define D __DATE__ __TIME__ __COUNTER__\nD H(D)\n";
    let r = || MemoryResolver::new().with_header("h.h", "#define H(x) [x] #x x ## 1\n");
    let a = dump(src, r());
    assert_eq!(a, dump(src, r()));
    assert!(a.contains("record complete"), "{a}");
}

/// `-E` and its diagnostics are the same with recording on and off.
#[test]
fn e_identical_on_and_off() {
    let srcs = [
        USES,
        "#define F(x, ...) x __VA_OPT__(+) __VA_ARGS__\nF(1) F(1,) F(1, 2, 3)\n#undef F\nF(1)\n",
        "#define P _Pragma(\"once\") __LINE__\n#pragma push_macro(\"P\")\n#undef P\nP\n#pragma pop_macro(\"P\")\nP\n",
        "#define A(x) x ## +\nA(+) A(-)\n#define B(\nB\n",
        "#include \"h.h\"\n#include \"h.h\"\n",
    ];
    for print in
        [PrintOptions::default(), PrintOptions { line_markers: true, ..PrintOptions::default() }]
    {
        for src in srcs {
            let r = || {
                MemoryResolver::new().with_header("h.h", "#define K(n) n,\nK(a) K(b)\n#undef K\n")
            };
            let off = run_with(src, r(), print, |_| {});
            let on = run_with(src, r(), print, |o| o.pp_record = true);
            assert_eq!(on.text, off.text, "-E of:\n{src}");
            assert_eq!(on.diags, off.diags, "diagnostics of:\n{src}");
        }
    }
}

/// A header's lines in the dump are the same under two main files that
/// differ before the inclusion.
#[test]
fn header_keys_under_two_mains() {
    let h = "#define S(x) (x + 1)\nS(S(2))\n";
    let mine = |d: &str| -> Vec<String> {
        d.lines()
            .filter(|l| l.split(' ').nth(1).is_some_and(|k| k.starts_with("s.h:")))
            .chain(d.lines().filter(|l| l.starts_with("tok ") && l.contains(" s.h:")))
            .map(str::to_owned)
            .collect()
    };
    let a = dump("#include \"s.h\"\n", MemoryResolver::new().with_header("s.h", h));
    let b = dump(
        "#define ONE 1\nint i = ONE + ONE;\n#include \"s.h\"\n",
        MemoryResolver::new().with_header("s.h", h),
    );
    let (ka, kb) = (mine(&a), mine(&b));
    assert!(ka.len() > 5, "{a}");
    assert_eq!(ka, kb);
}

/// Installing a preprocessor-state record makes the record incomplete.
#[test]
fn restored_state_is_a_gap() {
    let interner = Interner::new();
    let sm = SourceManager::new();
    let main = sm.add_virtual("t.cpp", "X\n".to_owned()).unwrap();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let mut opts = PpOptions::new(Std::Cxx26);
    opts.pp_record = true;
    let mut pp = Preprocessor::new(&sm, &engine, &interner, opts);
    pp.enter_main_file(main);
    assert!(pp.record().unwrap().is_complete());
    pp.install_state(&PpState::default(), Loc::INVALID);
    let rec = pp.record().unwrap();
    assert!(!rec.is_complete());
    assert!(rec.dump(&sm, &interner, &[]).ends_with("record incomplete: restored\n"));
}

/// Recording off: no record.
#[test]
fn off_by_default() {
    let interner = Interner::new();
    let sm = SourceManager::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let pp = Preprocessor::new(&sm, &engine, &interner, PpOptions::new(Std::Cxx26));
    assert!(pp.record().is_none());
}
