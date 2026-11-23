//! The preprocessing record (spec/pp.md "Preprocessing record"): the
//! parameter-use classifier against real expansions, one definition
//! expanded many times, stability, `-E` identity with recording on and
//! off, a header's keys under two main files, and the gap flags
//! ([cpp.replace], [cpp.subst], [cpp.stringize], [cpp.concat],
//! [cpp.rescan]); stage 2: conditionals tile their range, determinism of
//! the conditional and inclusion rows, include contexts ([cpp.cond],
//! [cpp.include]); stage 3: made tokens complete, paste operands, the
//! same tokens with recording on and off, condition reads, `cond_at` and
//! `enclosing`, push/pop pairs, the bytes counted, and the completeness
//! checks on a real translation unit ([cpp.concat], [cpp.stringize],
//! [cpp.cond], [cpp.pragma]).

mod common;

use common::{PREDEFINES, run_with};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_lex::{Interner, Std, TokKind};
use kiln_pp::state::PpState;
use kiln_pp::{
    ClangFeatures, CondIx, CondKind, CondOutcome, DefSource, DirGroup, ExpFlags, ExpIx,
    HeaderSearch, IncContext, IncKind, IncOutcome, MadeKind, MemoryResolver, Origin, ParamUse,
    PpOptions, PpRecord, Preprocessor, PrintOptions, ReadKind, RealFs, SearchDirSpec, SearchList,
    StackEvent, StackKind,
};
use kiln_source::{Loc, SourceManager};
use std::collections::HashSet;
use std::fmt::Write as _;
use std::time::Instant;

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
        COND,
        "#include \"g.h\"\n#include \"g.h\"\n#include \"o.h\"\n#include \"o.h\"\n#if 1\n",
    ];
    for print in
        [PrintOptions::default(), PrintOptions { line_markers: true, ..PrintOptions::default() }]
    {
        for src in srcs {
            let r = || {
                MemoryResolver::new()
                    .with_header("h.h", "#define K(n) n,\nK(a) K(b)\n#undef K\n")
                    .with_header("g.h", GUARDED)
                    .with_header("o.h", "#pragma once\nint o;\n")
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
    // The `in i<n>` field numbers instances within one translation unit:
    // it is not part of a key, so the comparison drops it.
    let mine = |d: &str| -> Vec<String> {
        d.lines()
            .filter(|l| l.split(' ').nth(1).is_some_and(|k| k.starts_with("s.h:")))
            .chain(d.lines().filter(|l| l.starts_with("tok ") && l.contains(" s.h:")))
            .map(|l| match l.rfind(" in i") {
                Some(i) => l[..i].to_owned(),
                None => l.to_owned(),
            })
            .collect()
    };
    let h = format!("{h}#if defined(S)\nint s;\n#elif 1\n#endif\n");
    let r = || MemoryResolver::new().with_header("s.h", &h).with_header("o.h", "int o;\n");
    let a = dump("#include \"s.h\"\n", r());
    let b = dump("#define ONE 1\nint i = ONE + ONE;\n#include \"o.h\"\n#include \"s.h\"\n", r());
    let (ka, kb) = (mine(&a), mine(&b));
    assert!(ka.len() > 5, "{a}");
    assert!(ka.iter().any(|l| l.starts_with("cond s.h:")), "{a}");
    assert!(b.contains(" in i2"), "{b}");
    assert_eq!(ka, kb);
}

/// A guarded header.
const GUARDED: &str = "#ifndef G_H\n#define G_H\nint g;\n#endif\n";

/// Conditionals of every kind: live, skipped, dead, not evaluated, a
/// spliced directive name, comments over two lines, and one the file ends
/// in.
const COND: &str = "\
#define TWO 2
#if 0 /* two
  lines */
int a;
#  ifdef X
#  elif Y
#  else
#  endif
#elif TWO == 2
int b;
#el\\
if 3
#else
#endif
#ifndef TWO
#elifdef TWO
#if 1 /* dead,
  two lines */
#endif
#elifndef Z
#endif
#ifdef TWO
#else
";

/// Each conditional tiles its range: from its opener's `#`, every byte is
/// in exactly one directive line or one group, in chain order, and the
/// last group of an unterminated one runs to the end of the file.
#[test]
fn conditionals_tile() {
    let src = format!("{COND}#endif\n{COND}");
    with_record(&src, MemoryResolver::new(), |sm, _, rec, _| {
        let mut seen = 0;
        for (i, c) in rec.conds().iter().enumerate() {
            if !c.kind().is_opener() {
                assert!(c.opener().is_some(), "row {i} has no opener");
                continue;
            }
            seen += 1;
            let mut pos = c.hash;
            let mut at = CondIx::from_index(i);
            loop {
                let x = rec.cond(at);
                assert_eq!(x.directive().lo, pos, "row {}", at.index());
                assert!(x.directive().hi >= x.directive().lo);
                pos = x.directive().hi;
                match rec.group(sm, at) {
                    Some(g) => {
                        assert_eq!(g.lo, pos);
                        assert!(g.hi >= g.lo);
                        pos = g.hi;
                    }
                    None => assert_eq!(x.kind(), CondKind::Endif),
                }
                match x.next() {
                    Some(n) => {
                        assert_eq!(rec.cond(n).opener(), Some(CondIx::from_index(i)));
                        at = n;
                    }
                    None => break,
                }
            }
            let last = rec.cond(at);
            if c.is_unterminated() {
                assert_ne!(last.kind(), CondKind::Endif);
                let (f, off) = sm.offset_of(pos).unwrap();
                assert_eq!(off as usize, sm.text(f).len(), "the last group ends the file");
            } else {
                assert_eq!(last.kind(), CondKind::Endif, "conditional at row {i}");
            }
        }
        assert!(seen >= 10, "{seen} conditionals");
        let unterminated = rec.conds().iter().filter(|c| c.is_unterminated()).count();
        assert_eq!(unterminated, 1);
        let dead = rec.conds().iter().filter(|c| c.outcome() == CondOutcome::Dead).count();
        let not_evaluated =
            rec.conds().iter().filter(|c| c.outcome() == CondOutcome::NotEvaluated).count();
        assert!(dead >= 8 && not_evaluated >= 2, "dead {dead}, not evaluated {not_evaluated}");
    });
}

/// A header entered twice has its rows twice, each instance's rows
/// together and in directive order, and two runs give the same rows.
#[test]
fn rows_per_instance_and_stable() {
    let src = "#include \"c.h\"\n#define K 1\n#include \"c.h\"\n";
    let r =
        || MemoryResolver::new().with_header("c.h", "#ifdef K\nint k;\n#else\nint n;\n#endif\n");
    let a = dump(src, r());
    assert_eq!(a, dump(src, r()));
    with_record(src, r(), |sm, _, rec, _| {
        let entered: Vec<_> = rec
            .incs()
            .iter()
            .filter_map(|x| match x.outcome {
                IncOutcome::Entered { base } => Some(base),
                _ => None,
            })
            .collect();
        assert_eq!(entered.len(), 2);
        assert_eq!(rec.conds().len(), 6);
        for (k, base) in entered.iter().enumerate() {
            let rows = &rec.conds()[3 * k..3 * k + 3];
            let kinds: Vec<_> = rows.iter().map(kiln_pp::Conditional::kind).collect();
            assert_eq!(kinds, [CondKind::Ifdef, CondKind::Else, CondKind::Endif]);
            for c in rows {
                let IncContext::Inclusion(i) = rec.inclusion_of(sm, c.hash) else {
                    panic!("a header row outside an inclusion");
                };
                assert_eq!(rec.inc(i).outcome, IncOutcome::Entered { base: *base });
            }
        }
        let first: Vec<_> = rec.conds()[..3].iter().map(kiln_pp::Conditional::outcome).collect();
        let second: Vec<_> = rec.conds()[3..].iter().map(kiln_pp::Conditional::outcome).collect();
        assert_eq!(first, [CondOutcome::NotTaken, CondOutcome::Taken, CondOutcome::Closes]);
        assert_eq!(second, [CondOutcome::Taken, CondOutcome::NotTaken, CondOutcome::Closes]);
        // The same keys: equal offsets in each instance's range.
        let off = |c: &kiln_pp::Conditional| sm.offset_of(c.hash).unwrap().1;
        for k in 0..3 {
            assert_eq!(off(&rec.conds()[k]), off(&rec.conds()[k + 3]));
        }
    });
}

/// `inclusion_of` is total: the main file is `Main`, the predefines
/// `Builtin`, a header its instance, a macro location the context of its
/// outermost invocation, and anything else `Unknown`; a nested instance's
/// chain walks out to the main file's directive.
#[test]
fn include_contexts() {
    let src = "#define M(x) x\n#include \"w.h\"\nint m = M(1);\n";
    let r = MemoryResolver::new()
        .with_header("w.h", "#include \"c.h\"\nint w = M(2);\n")
        .with_header("c.h", "int c = M(3);\n");
    with_record(src, r, |sm, _, rec, toks| {
        let main = rec.incs()[0].hash;
        assert_eq!(rec.inclusion_of(sm, main), IncContext::Main);
        assert_eq!(rec.inclusion_of(sm, Loc::INVALID), IncContext::Unknown);
        let builtin = rec.defs().iter().find(|d| d.source == DefSource::Predefined).unwrap();
        assert_eq!(rec.inclusion_of(sm, builtin.hash), IncContext::Builtin);
        assert_eq!(rec.incs().len(), 2);
        let (w, c) = (rec.incs()[0].clone(), rec.incs()[1].clone());
        assert_eq!((w.kind, c.kind), (IncKind::Include, IncKind::Include));
        assert_eq!(w.parent(), None);
        let IncContext::Inclusion(wi) = rec.inclusion_of(sm, c.hash) else {
            panic!("c's directive")
        };
        assert_eq!(c.parent(), Some(wi));
        let ci = kiln_pp::IncIx::from_index(1);
        let chain: Vec<_> = rec.context(ci).map(kiln_pp::IncIx::index).collect();
        assert_eq!(chain, [1, 0]);
        assert_eq!(rec.instances_of(c.file.unwrap()).count(), 1);
        // Each `M(...)` result token's context is that of its invocation.
        let want = [IncContext::Inclusion(ci), IncContext::Inclusion(wi), IncContext::Main];
        let got: Vec<_> = toks
            .iter()
            .filter(|(l, _)| sm.is_macro_loc(*l))
            .map(|(l, _)| rec.inclusion_of(sm, *l))
            .collect();
        assert_eq!(got, want);
    });
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

// ---- stage 3: made tokens, condition reads, queries, push/pop ----

/// Macros and conditions that use every stage-3 table: pastes (a chain,
/// placemarkers, a forwarded argument, a pasted operand, `#x ## y`, an
/// invalid one), `#__VA_OPT__`, `#__VA_ARGS__`, conditions with every
/// kind of read, feature queries and `push_macro`/`pop_macro`.
const STAGE3: &str = "\
#define CAT(a, b) a ## b
#define XCAT(a, b) CAT(a, b)
#define CAT3(a, b, c) a ## b ## c
#define UDL(x) #x ## _s
#define VS(...) #__VA_OPT__(a ## __VA_ARGS__ b)
#define S(...) #__VA_ARGS__
#define BAD(a) a ## +
#define D defined(E)
#define F(x) x
int CAT3(x, y, z), CAT3(, q, ), XCAT(CAT(p, q), r);
long operator\"\"_s(const char *, decltype(sizeof 0));
long u = UDL(w);
const char *s1 = VS(1 2), *s2 = VS(), *s3 = S(a, b);
int bad = BAD(1) 2;
#if defined(A) && B > 2 || 0 && defined(C)
#elif F(G) + H
#elif D
#endif
#ifdef A
#elifndef A
#endif
#if __has_include(\"h.h\") && __has_cpp_attribute(nodiscard) && __has_feature(cxx_rvalue_references)
#endif
#if 1 + 2 && true
#endif
#if V W
#endif
#pragma push_macro(\"F\")
#undef F
#pragma pop_macro(\"F\")
#pragma pop_macro(\"F\")
_Pragma(\"push_macro(\\\"D\\\")\") _Pragma(\"pop_macro(\\\"D\\\")\")
";

/// A condition with no identifier but `true`/`false` (after removing
/// comments, literals with their encoding prefixes, and pp-numbers): it
/// reads nothing.
fn literally_constant(text: &str) -> bool {
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
        } else if c == b'/' && b.get(i + 1) == Some(&b'/') {
            break;
        } else if c == b'\'' || c == b'"' {
            i += 1;
            while i < b.len() && b[i] != c {
                i += if b[i] == b'\\' { 2 } else { 1 };
            }
            i += 1;
        } else if c.is_ascii_digit() {
            while i < b.len()
                && (b[i].is_ascii_alphanumeric() || matches!(b[i], b'_' | b'.' | b'\''))
            {
                i += 1;
            }
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let s = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let prefix = matches!(&text[s..i], "L" | "u" | "U" | "u8")
                && matches!(b.get(i), Some(b'\'' | b'"'));
            if !prefix && !matches!(&text[s..i], "true" | "false") {
                return false;
            }
        } else {
            i += 1;
        }
    }
    true
}

/// The directive's text after its name (`#  if` -> the condition).
fn condition_text(sm: &SourceManager, d: kiln_source::Span) -> &str {
    let (f, lo) = sm.offset_of(d.lo).unwrap();
    let (_, hi) = sm.offset_of(d.hi).unwrap();
    let t = &sm.text(f)[lo as usize..hi as usize];
    let t = t.trim_start_matches('#').trim_start();
    t.trim_start_matches(|c: char| c.is_ascii_alphabetic())
}

/// A host run's tokens and, with recording on, its coverage and
/// `bytes()`.
type HostRun = (Vec<String>, Option<(Coverage, usize)>);

/// What [`check_complete`] saw.
#[derive(Debug, Default)]
struct Coverage {
    made_tokens: usize,
    made_rows: usize,
    evaluated: usize,
    reads: usize,
    constant: usize,
    partial: usize,
}

/// The stage-3 completeness checks (spec/pp.md "Preprocessing record",
/// coverage): every token spelled in scratch space derives to a made
/// entry with its derivation (a paste's operands, a `#x`'s argument, a
/// `#__VA_OPT__`'s operands, or a builtin), every made entry answers
/// `made` and is listed by `mades_of` its expansion; every evaluated
/// condition has reads, is partial or is literally constant, each
/// `#ifdef` form has exactly one `tested` read, a condition not evaluated
/// has none, and the `expanded` reads are exactly the expansions entered
/// in the directive.
fn check_complete(sm: &SourceManager, rec: &PpRecord, toks: &[Loc]) -> Coverage {
    let mut cov = Coverage::default();
    for &t in toks {
        if !sm.is_macro_loc(t) {
            continue;
        }
        let scratch = sm.is_written_in_scratch_space(sm.spelling_loc(t));
        let mut l = t;
        let made = loop {
            match rec.origin(sm, l) {
                Origin::Made { .. } => break true,
                Origin::Arg { from, .. } if sm.is_macro_loc(from) => l = from,
                Origin::Arg { .. } | Origin::Body { .. } | Origin::File(_) => break false,
                Origin::Unknown => panic!("a token the record did not see made"),
            }
        };
        if scratch {
            assert!(made, "a scratch-space token with no made origin");
            cov.made_tokens += 1;
        }
    }
    for (k, base) in rec.mades().enumerate() {
        let m = rec.made(sm, base).expect("a made entry answers made()");
        match m.kind {
            MadeKind::Paste => {
                assert!(m.operands.len() >= 2, "a paste with {} operands", m.operands.len());
                cov.made_rows += 1;
                for &op in m.operands {
                    assert!(sm.is_macro_loc(op), "an operand is a macro location");
                }
            }
            MadeKind::Stringize if m.va_opt => cov.made_rows += 1,
            MadeKind::Stringize => assert!(m.arg.is_some(), "a `#x` without its argument"),
            _ => assert!(m.operands.is_empty()),
        }
        // `mades_of` is a scan: check a prefix.
        if let Some(e) = m.exp
            && k < 300
        {
            assert!(rec.mades_of(e).any(|b| b == base), "mades_of misses a made entry");
        }
    }
    // The `IN_DIRECTIVE` expansions by the expansion location of their
    // name.
    let mut in_dir: Vec<(Loc, ExpIx)> = (0..rec.exps().len())
        .map(ExpIx::from_index)
        .filter(|&e| rec.flags(e).contains(ExpFlags::IN_DIRECTIVE))
        .map(|e| (sm.expansion_loc(rec.exp(e).name), e))
        .collect();
    in_dir.sort_unstable_by_key(|&(at, e)| (at, e.index()));
    for (i, c) in rec.conds().iter().enumerate() {
        let ix = CondIx::from_index(i);
        let reads = rec.reads(ix);
        let evaluated = !matches!(c.kind(), CondKind::Else | CondKind::Endif)
            && matches!(c.outcome(), CondOutcome::Taken | CondOutcome::NotTaken);
        if !evaluated {
            assert!(reads.is_empty() && !rec.reads_partial(ix), "reads of an unevaluated row");
            continue;
        }
        cov.evaluated += 1;
        cov.reads += reads.len();
        let partial = rec.reads_partial(ix);
        cov.partial += usize::from(partial);
        let text = condition_text(sm, c.directive());
        if reads.is_empty() && !partial {
            assert!(literally_constant(text), "no reads for `{text}`");
            cov.constant += 1;
        }
        let ifdef_form = matches!(
            c.kind(),
            CondKind::Ifdef | CondKind::Ifndef | CondKind::Elifdef | CondKind::Elifndef
        );
        if ifdef_form && !partial {
            assert_eq!(reads.len(), 1, "`{text}`");
            assert_eq!(reads[0].kind(), ReadKind::Tested);
        }
        let d = c.directive();
        let lo = in_dir.partition_point(|&(at, _)| at < d.lo);
        let hi = in_dir.partition_point(|&(at, _)| at < d.hi);
        let mut entered: Vec<ExpIx> = in_dir[lo..hi].iter().map(|&(_, e)| e).collect();
        entered.sort_unstable_by_key(|e| e.index());
        let expanded: Vec<ExpIx> = reads
            .iter()
            .filter_map(|r| match r.kind() {
                ReadKind::Expanded(e) => Some(e),
                _ => None,
            })
            .collect();
        assert_eq!(expanded, entered, "the expansions of `{text}`");
    }
    cov
}

/// The token locations of [`with_record`]'s tokens.
fn locs(toks: &[(Loc, String)]) -> Vec<Loc> {
    toks.iter().map(|t| t.0).collect()
}

fn stage3_headers() -> MemoryResolver {
    MemoryResolver::new()
        .with_header("h.h", "#define K(n) n,\nK(a) K(b)\n#undef K\n")
        .with_header("g.h", GUARDED)
}

/// [cpp.concat], [cpp.stringize], [cpp.subst]/3, [cpp.cond]: the stage-3
/// completeness checks over every table's cases.
#[test]
fn stage3_complete() {
    with_record(STAGE3, stage3_headers(), |sm, _, rec, toks| {
        let cov = check_complete(sm, rec, &locs(toks));
        assert!(cov.made_tokens >= 6 && cov.made_rows >= 6, "{cov:?}");
        assert!(cov.evaluated >= 8 && cov.constant >= 1 && cov.partial >= 1, "{cov:?}");
        assert!(rec.queries().len() >= 3);
        assert_eq!(rec.stack_events().len(), 5);
    });
    let src = format!("{USES}{COND}");
    with_record(&src, MemoryResolver::new(), |sm, _, rec, toks| {
        check_complete(sm, rec, &locs(toks));
    });
}

/// [cpp.concat]/3: the operands are the tokens pasted: for `CAT(ab, cd)`
/// they are spelled `ab` and `cd`, a pasted operand is itself made, and a
/// placemarker paste leaves the surviving token's origin unchanged.
#[test]
fn paste_operands() {
    let src = "#define CAT(a, b) a ## b\n#define XCAT(a, b) CAT(a, b)\n\
               int CAT(ab, cd);\nint XCAT(CAT(p, q), r);\nint CAT(, solo);\n";
    with_record(src, MemoryResolver::new(), |sm, _, rec, toks| {
        let at = |s: &str| toks.iter().find(|t| t.1 == s).unwrap().0;
        let spelled = |l: Loc| {
            let (f, o) = sm.offset_of(sm.spelling_loc(l)).unwrap();
            let t = &sm.text(f)[o as usize..];
            t[..t.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(t.len())].to_owned()
        };
        let m = rec.made(sm, at("abcd")).unwrap();
        assert_eq!(m.kind, MadeKind::Paste);
        let ops: Vec<_> = m.operands.iter().map(|&o| spelled(o)).collect();
        assert_eq!(ops, ["ab", "cd"]);
        let m = rec.made(sm, at("pqr")).unwrap();
        assert_eq!(m.operands.len(), 2);
        // The operand is the substituted argument token (forwarded by
        // `XCAT`): following `from` reaches the token the inner paste made.
        let mut op = m.operands[0];
        assert!(rec.made(sm, op).is_none());
        while let Origin::Arg { from, .. } = rec.origin(sm, op) {
            op = from;
        }
        let inner = rec.made(sm, op).expect("the pasted operand is made");
        assert_eq!(rec.made_text(sm, inner.base), Some("pq"));
        assert_eq!(spelled(m.operands[1]), "r");
        let solo = at("solo");
        assert!(rec.made(sm, solo).is_none());
        assert!(matches!(rec.origin(sm, solo), Origin::Arg { arg: 1, .. }));
        assert_eq!(rec.mades_of(m.exp.unwrap()).count(), 1);
    });
}

/// Recording on and off give the same tokens: every location, kind and
/// symbol (the record interns nothing and allocates no location).
#[test]
fn same_tokens_on_and_off() {
    let run = |src: &str, on: bool| {
        let interner = Interner::new();
        let sm = SourceManager::new();
        let main = sm.add_virtual("t.cpp", src.to_owned()).unwrap();
        let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
        let mut opts = PpOptions::new(Std::Cxx26);
        PREDEFINES.clone_into(&mut opts.predefines);
        opts.reproducible_time = Some(("Jan  1 2000".to_owned(), "00:00:00".to_owned()));
        opts.pp_record = on;
        let mut pp = Preprocessor::new(&sm, &engine, &interner, opts);
        pp.set_include_resolver(Box::new(stage3_headers()));
        pp.enter_main_file(main);
        let mut v = Vec::new();
        loop {
            let t = pp.next();
            if t.kind == TokKind::Eof {
                break;
            }
            v.push(format!("{} {} {:?}", t.span.lo.to_raw(), t.span.hi.to_raw(), t.kind));
        }
        v.push(format!("symbols {}", interner.intern("zz_last_symbol").index()));
        v
    };
    for src in [STAGE3, USES, COND] {
        assert_eq!(run(src, true), run(src, false), "{src}");
    }
}

/// [cpp.cond]/1, [cpp.include]: `cond_at` gives the innermost group
/// holding a location, in a skipped group too, each instance of a header
/// its own rows, a macro location through its expansion location, and
/// `None` on a directive line or outside every group; `enclosing` walks
/// out.
#[test]
fn cond_at_and_enclosing() {
    let h =
        "#ifdef K\nint k;\n#if 0\nint dead;\n#endif\nint k2;\n#else\nint n;\n#endif\nint after;\n";
    let src = "#include \"c.h\"\n#define K 1\n#include \"c.h\"\n#define M(x) x\n#if 1\nint M(y);\n#endif\n";
    let r = MemoryResolver::new().with_header("c.h", h);
    with_record(src, r, |sm, _, rec, toks| {
        let off = |s: &str| u32::try_from(h.find(s).unwrap()).unwrap();
        let row = |l: Loc| {
            let i = rec.conds().iter().position(|c| c.hash == l).unwrap();
            CondIx::from_index(i)
        };
        let bases: Vec<Loc> = rec
            .incs()
            .iter()
            .filter_map(|x| match x.outcome {
                IncOutcome::Entered { base } => Some(base),
                _ => None,
            })
            .collect();
        assert_eq!(bases.len(), 2);
        let mut seen = HashSet::new();
        for &b in &bases {
            let at = |s: &str| b.with_offset(off(s));
            let (ifdef, if0, els) = (row(at("#ifdef")), row(at("#if 0")), row(at("#else")));
            assert!(seen.insert(ifdef.index()), "each instance has its own rows");
            assert_eq!(rec.cond_at(sm, at("int k;")), Some(ifdef));
            assert_eq!(rec.cond_at(sm, at("int dead")), Some(if0));
            assert_eq!(rec.cond_at(sm, at("int k2")), Some(ifdef));
            assert_eq!(rec.cond_at(sm, at("int n")), Some(els));
            assert_eq!(rec.cond_at(sm, at("int after")), None);
            assert_eq!(rec.cond_at(sm, at("#if 0")), Some(ifdef), "a nested directive");
            assert_eq!(rec.cond_at(sm, at("#ifdef")), None, "the opener's own line");
            assert_eq!(rec.enclosing(if0), Some(ifdef));
            assert_eq!(rec.enclosing(row(at("#endif\nint k2"))), Some(ifdef));
            assert_eq!(rec.enclosing(ifdef), None);
            assert_eq!(rec.enclosing(els), None);
        }
        let y = toks.iter().find(|t| t.1 == "y").unwrap().0;
        assert!(sm.is_macro_loc(y));
        let main_if = rec.cond_at(sm, y).unwrap();
        assert_eq!(rec.cond(main_if).kind(), CondKind::If);
        assert_eq!(rec.inclusion_of(sm, rec.cond(main_if).hash), IncContext::Main);
        assert_eq!(rec.cond_at(sm, Loc::INVALID), None);
    });
}

/// [cpp.pragma]: each `pop_macro` names the `push_macro` it undid, nested
/// pushes of one name pop in reverse, other names do not interfere, and a
/// pop with nothing pushed is `PopUnmatched`.
#[test]
fn push_and_pop_pairs() {
    use StackKind::{Pop, PopUnmatched, Push};
    let src = "#define A 1\n#pragma push_macro(\"A\")\n#pragma push_macro(\"B\")\n\
               #pragma push_macro(\"A\")\n#undef A\n#pragma pop_macro(\"A\")\n\
               #pragma pop_macro(\"B\")\n#pragma pop_macro(\"A\")\n#pragma pop_macro(\"A\")\n";
    with_record(src, MemoryResolver::new(), |_, interner, rec, _| {
        let ev = rec.stack_events();
        let kinds: Vec<_> = ev.iter().map(StackEvent::kind).collect();
        assert_eq!(kinds, [Push, Push, Push, Pop, Pop, Pop, PopUnmatched]);
        let pairs: Vec<_> = (0..ev.len()).map(|i| rec.push_of(i)).collect();
        assert_eq!(pairs, [None, None, None, Some(2), Some(1), Some(0), None]);
        let name = |d: Option<kiln_pp::DefIx>| d.map(|d| interner.as_str(rec.def(d).def.name()));
        assert_eq!(name(ev[0].def()), Some("A"));
        assert_eq!(ev[3].def(), ev[0].def(), "restores the saved definition");
        assert_eq!(ev[3].replaced(), None, "A was undefined");
        assert_eq!(ev[4].def(), None, "B was never defined");
        assert!(ev.iter().all(|e| e.exp().is_none()));
    });
}

/// The new tables are counted in `bytes()` (the memory report's row).
#[test]
fn stage3_bytes() {
    let base = "#define X 1\n";
    let mut src = base.to_owned();
    for _ in 0..200 {
        src.push_str("#pragma push_macro(\"X\")\n#if defined(X) && X\n#endif\n");
    }
    let b0 = with_record(base, MemoryResolver::new(), |_, _, rec, _| rec.bytes());
    with_record(&src, MemoryResolver::new(), |_, _, rec, _| {
        assert_eq!(rec.stack_events().len(), 200);
        let reads: usize =
            (0..rec.conds().len()).map(|i| rec.reads(CondIx::from_index(i)).len()).sum();
        assert_eq!(reads, 400);
        let floor = 200 * size_of::<StackEvent>() + reads * size_of::<kiln_pp::Read>();
        assert!(rec.bytes() >= b0 + floor, "{} < {b0} + {floor}", rec.bytes());
    });
}

/// The host's libstdc++ search list and predefines (spec/pp.md
/// "Preprocessing record", coverage); `None` without libstdc++ headers.
fn host_search() -> Option<(&'static str, Vec<SearchDirSpec>, String)> {
    let triple = match std::env::consts::ARCH {
        "aarch64" => "aarch64-unknown-linux-gnu",
        "x86_64" => "x86_64-unknown-linux-gnu",
        _ => return None,
    };
    let resource = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resource");
    let list =
        std::fs::read_to_string(resource.join(format!("include-paths/{triple}.txt"))).ok()?;
    let res_inc = resource.join("include").display().to_string();
    let mut dirs = Vec::new();
    let mut on = false;
    for line in list.lines() {
        if line.starts_with("#include <...>") {
            on = true;
        } else if line.starts_with("End of search list") {
            on = false;
        } else if on && let Some(p) = line.strip_prefix(' ') {
            let p =
                if p.ends_with("/lib/clang/19/include") { res_inc.clone() } else { p.to_owned() };
            dirs.push(SearchDirSpec::new(p, DirGroup::System));
        }
    }
    if !dirs.iter().any(|d| std::path::Path::new(&d.path).join("vector").is_file()) {
        return None;
    }
    let pre =
        std::fs::read_to_string(resource.join(format!("predefines/{triple}.c++26.txt"))).ok()?;
    Some((triple, dirs, pre))
}

/// Runs `src` against the host's headers (plus `extra` `-I` directories
/// and `#define` lines) with recording `on`, returning the tokens'
/// locations and kinds, the record's coverage and `bytes()`.
fn host_run(src: &str, extra: &[String], defines: &str, on: bool) -> Option<HostRun> {
    let (triple, mut dirs, pre) = host_search()?;
    let mut angled: Vec<_> =
        extra.iter().map(|p| SearchDirSpec::new(p.clone(), DirGroup::Angled)).collect();
    angled.append(&mut dirs);
    let interner = Interner::new();
    let sm = SourceManager::new();
    let main = sm.add_virtual("t.cpp", src.to_owned()).unwrap();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let mut opts = PpOptions::new(Std::Cxx26);
    opts.predefines = format!("{pre}{defines}");
    opts.reproducible_time = Some(("Jan  1 2000".to_owned(), "00:00:00".to_owned()));
    opts.pp_record = on;
    let mut pp = Preprocessor::new(&sm, &engine, &interner, opts);
    let mut fs = RealFs::new();
    let list = SearchList::build(&angled, true, &mut fs);
    pp.set_include_resolver(Box::new(HeaderSearch::new(fs, list)));
    pp.set_feature_queries(Box::new(ClangFeatures::new(triple, Std::Cxx26, &[]).unwrap()));
    pp.enter_main_file(main);
    let mut seen = Vec::new();
    let mut toks = Vec::new();
    loop {
        let t = pp.next();
        if t.kind == TokKind::Eof {
            break;
        }
        seen.push(t.span.lo);
        toks.push(format!("{} {:?}", t.span.lo.to_raw(), t.kind));
    }
    toks.push(format!("symbols {}", interner.intern("zz_last_symbol").index()));
    let cov = pp.record().map(|rec| {
        assert!(rec.is_complete());
        (check_complete(&sm, rec, &seen), rec.bytes())
    });
    Some((toks, cov))
}

fn host_tu(src: &str, extra: &[String], defines: &str) {
    let t = Instant::now();
    let Some((on, cov)) = host_run(src, extra, defines, true) else {
        println!("skipped: no libstdc++ headers on this host");
        return;
    };
    let t_on = t.elapsed();
    let (off, _) = host_run(src, extra, defines, false).unwrap();
    assert_eq!(on.len(), off.len());
    assert!(on == off, "tokens differ with recording on and off");
    let (cov, bytes) = cov.unwrap();
    println!("{} tokens, {cov:?}, record {bytes} B, {t_on:?} with recording", on.len());
    assert!(cov.made_tokens > 0 && cov.evaluated > 1000 && cov.reads > 1000, "{cov:?}");
}

/// The completeness checks on a real translation unit: `<bits/stdc++.h>`
/// (skipped without libstdc++ headers).
#[test]
fn real_tu_complete() {
    host_tu("#include <bits/stdc++.h>\n", &[], "");
}

/// The completeness checks on any file: `KILN_PP_RECORD_TU=<file>` with
/// `KILN_PP_RECORD_ARGS` holding its `-I<dir>` and `-D<name>[=<value>]`
/// (skipped when unset; used for large real sources).
#[test]
fn real_tu_from_env() {
    let Ok(file) = std::env::var("KILN_PP_RECORD_TU") else { return };
    let args = std::env::var("KILN_PP_RECORD_ARGS").unwrap_or_default();
    let mut dirs = Vec::new();
    let mut defines = String::new();
    for a in args.split_whitespace() {
        if let Some(d) = a.strip_prefix("-I") {
            dirs.push(d.to_owned());
        } else if let Some(d) = a.strip_prefix("-D") {
            let (n, v) = d.split_once('=').unwrap_or((d, "1"));
            let _ = writeln!(defines, "#define {n} {v}");
        }
    }
    host_tu(&format!("#include \"{file}\"\n"), &dirs, &defines);
}
