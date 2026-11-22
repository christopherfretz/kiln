//! WP-2072: `constexpr` entries of evaluation sessions
//! (spec/eval-session.md §3, §4): a `constexpr` function runs under the
//! rules of a `consteval` one; its run is the manifestly
//! constant-evaluated initialization of a notional `constexpr` variable
//! at the end of the translation unit ([expr.const.defns]/1.6), not an
//! immediate invocation.
#![forbid(unsafe_code)]

use std::sync::Arc;

use kiln_diag::{DiagEngine, DiagOptions, Diagnostic, MemorySink};
use kiln_parse::{
    CancelToken, ConstexprLimits, EntryError, EvalSession, HostCalls, HostError, HostOut,
    ParseOptions, RunBudget, RunError, RunReport, RunValue, parse_preprocessed_retained,
};
use kiln_pp::{ClangFeatures, ModuleOptions, PpOptions, Preprocessor};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// The wrapper every test source starts with.
const PRE: &str = "using u64 = unsigned long long;\n\
constexpr u64 hc(unsigned op) {\n\
return __builtin_kiln_host_call(op, nullptr, 0, nullptr, 0, nullptr, 0, nullptr, 0);\n\
}\n";

/// A run's handler: op 1 returns `n`; anything else fails.
struct Give(u64);

impl HostCalls for Give {
    fn call(
        &mut self,
        op: u32,
        _ints: &[u64],
        _bytes: &[u8],
        _out: &mut HostOut<'_>,
    ) -> Result<u64, HostError> {
        match op {
            1 => Ok(self.0),
            _ => Err(HostError::new(format!("unknown op {op}"))),
        }
    }
}

/// Each diagnostic and note of `ds` as `L:C message`.
fn render(sm: &SourceManager, ds: &[Diagnostic]) -> Vec<String> {
    fn one(sm: &SourceManager, d: &Diagnostic, out: &mut Vec<String>) {
        let pos = sm
            .line_col(d.span.lo)
            .map_or_else(|| "-".to_owned(), |lc| format!("{}:{}", lc.line, lc.col));
        out.push(format!("{pos} {}", d.message));
        for n in &d.notes {
            one(sm, n, out);
        }
    }
    let mut out = Vec::new();
    for d in ds {
        one(sm, d, &mut out);
    }
    out
}

/// Parses `PRE` + `src`, opens a session and calls `f` with it.
fn session<R>(
    src: &str,
    f: impl for<'a, 'sm> FnOnce(&mut EvalSession<'a, 'sm>, &'sm SourceManager) -> R,
) -> R {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", format!("{PRE}{src}")).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let opts = ParseOptions::default();
    let dopts = DiagOptions { clang_compat: opts.clang_compat, ..DiagOptions::default() };
    let engine = DiagEngine::new(&sm, &dopts, sink.clone());
    let mut ppo = PpOptions::new(opts.std);
    ppo.parser_pragmas = true;
    ppo.modules = Some(ModuleOptions::default());
    let mut pp = Preprocessor::new(&sm, &engine, &interner, ppo);
    let features = ClangFeatures::new("x86_64-unknown-linux-gnu", opts.std, &[])
        .expect("x86-64 features")
        .with_kiln_host_calls(true);
    pp.set_feature_queries(Box::new(features));
    pp.enter_main_file(file);
    let (sema, _pp) = parse_preprocessed_retained(
        pp,
        Arc::clone(&interner),
        &engine,
        Target::for_arch(Arch::X86_64),
        &opts,
        ConstexprLimits::default(),
        Some(Box::new(Give(1000))),
    );
    let tu = render(&sm, &sink.take());
    assert!(tu.is_empty(), "translation unit diagnostics:\n{}", tu.join("\n"));
    let mut s = EvalSession::open(sema).expect("session opens");
    let r = f(&mut s, &sm);
    let late = render(&sm, &sink.take());
    assert!(late.is_empty(), "runs' diagnostics reached the sink:\n{}", late.join("\n"));
    r
}

/// What a run reports, as compared between runs.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Seen {
    result: Result<RunValue, RunError>,
    steps: u64,
    nested_steps: u64,
    calls: u64,
    nested_calls: u64,
    diags: Vec<String>,
}

fn seen(sm: &SourceManager, r: &RunReport) -> Seen {
    Seen {
        result: r.result.clone(),
        steps: r.steps,
        nested_steps: r.nested_steps,
        calls: r.calls,
        nested_calls: r.nested_calls,
        diags: render(sm, &r.diagnostics),
    }
}

fn run_with(s: &mut EvalSession<'_, '_>, sm: &SourceManager, name: &str, host: u64) -> Seen {
    let e = s.entry_named(name).unwrap_or_else(|e| panic!("entry {name}: {e}"));
    let r = s.run(e, &mut Give(host), RunBudget::PROGRAM, &CancelToken::new());
    seen(sm, &r)
}

fn run(s: &mut EvalSession<'_, '_>, sm: &SourceManager, name: &str) -> Seen {
    run_with(s, sm, name, 0)
}

#[track_caller]
fn ok(r: &Seen) -> RunValue {
    assert!(r.diags.is_empty(), "unexpected diagnostics: {:?}", r.diags);
    match &r.result {
        Ok(v) => v.clone(),
        Err(e) => panic!("run failed: {e:?}"),
    }
}

/// The declaration of the specialization of function template `name`
/// (at namespace scope) whose position among its specializations is
/// `nth`, as a client holding a `DeclId` has it.
fn spec_of(s: &EvalSession<'_, '_>, name: &str, nth: usize) -> kiln_ast::DeclId {
    let sema = s.sema();
    let sym = sema.ast.symbols().intern(name);
    let found = sema.lookup_qualified(sema.ast.tu, sym, kiln_sema::lookup::LookupKind::Ordinary);
    let [t] = found.decls.as_slice() else { panic!("{name}: {:?}", found.decls) };
    let specs = sema.ast.templates.specs_of(sema.ast.first(*t));
    specs.get(nth).copied().unwrap_or_else(|| panic!("{name}: {} specializations", specs.len()))
}

const KINDS: &str = "constexpr int neg() { return -7; }\n\
constexpr bool yes() { return 3 > 2; }\n\
constexpr unsigned long long big() { return ~0ull; }\n\
int sink;\n\
constexpr void nothing() { int a[3]{1, 2, 3}; for (int& x : a) x *= 2; }\n\
namespace n { struct S { static constexpr int f() { return 5; } }; }\n\
template <class T> constexpr int size() { return sizeof(T) * 10; }\n\
template <class T> consteval int csize() { return sizeof(T) * 100; }\n\
static_assert(size<char>() == 10 && size<int>() == 40);\n\
static_assert(csize<short>() == 200);\n\
int plain() { return 1; }\n\
struct P { static int g() { return 2; } constexpr int h() const { return 3; } };\n\
template <class T> int tplain() { return 4; }\n\
int use_tplain = tplain<int>();\n";

#[test]
fn constexpr_entries_of_each_kind() {
    session(KINDS, |s, sm| {
        assert_eq!(ok(&run(s, sm, "neg")), RunValue::Signed(-7));
        assert_eq!(ok(&run(s, sm, "yes")), RunValue::Bool(true));
        assert_eq!(ok(&run(s, sm, "big")), RunValue::Unsigned(u128::from(u64::MAX)));
        assert_eq!(ok(&run(s, sm, "nothing")), RunValue::Void);
        assert_eq!(ok(&run(s, sm, "n::S::f")), RunValue::Signed(5));
        // Function template specializations, by declaration.
        for (name, nth, v) in [("size", 0, 10), ("size", 1, 40), ("csize", 0, 200)] {
            let e = s.entry(spec_of(s, name, nth)).unwrap_or_else(|e| panic!("{name}: {e}"));
            let r = s.run(e, &mut Give(0), RunBudget::PROGRAM, &CancelToken::new());
            assert_eq!(ok(&seen(sm, &r)), RunValue::Signed(v), "{name} #{nth}");
        }
        // Neither constexpr nor consteval ([dcl.constexpr]/1).
        let err = |s: &EvalSession<'_, '_>, name: &str| s.entry_named(name).unwrap_err();
        assert_eq!(err(s, "plain"), EntryError::NotConstexpr);
        assert_eq!(err(s, "P::g"), EntryError::NotConstexpr);
        assert_eq!(s.entry(spec_of(s, "tplain", 0)), Err(EntryError::NotConstexpr));
        assert!(err(s, "plain").to_string().contains("neither a constexpr nor a consteval"));
        // The other rules of §3 hold of constexpr entries as well.
        assert_eq!(err(s, "P::h"), EntryError::HasParameters);
        assert_eq!(err(s, "size"), EntryError::NotAFunction);
    });
}

/// `std::is_constant_evaluated` as libstdc++ and libc++ define it.
const ICE: &str = "namespace std { constexpr bool is_constant_evaluated() noexcept { \
return __builtin_is_constant_evaluated(); } }\n\
constexpr int at(int i) { int a[2]{}; return a[i]; }\n\
constexpr int how() { if consteval { return 1; } else { return 2; } }\n\
constexpr int std_ice() { return std::is_constant_evaluated() ? 10 : 20; }\n\
constexpr int builtin_ice() { return __builtin_is_constant_evaluated() ? 100 : 200; }\n\
constexpr int all() { return how() + std_ice() + builtin_ice(); }\n\
constexpr int plainly() { return __kiln_plainly_constant_evaluated(); }\n\
constexpr void vcheck() { if !consteval { at(5); } }\n\
consteval int call_all() { return all(); }\n\
consteval int call_plainly() { return plainly(); }\n";

#[test]
fn a_run_is_manifestly_constant_evaluated() {
    // [expr.const.defns]/1.6 (the notional constexpr variable's
    // initializer), [stmt.if]/5, [meta.const.eval]/1.
    session(ICE, |s, sm| {
        for _ in 0..2 {
            assert_eq!(ok(&run(s, sm, "how")), RunValue::Signed(1));
            assert_eq!(ok(&run(s, sm, "std_ice")), RunValue::Signed(10));
            assert_eq!(ok(&run(s, sm, "builtin_ice")), RunValue::Signed(100));
            assert_eq!(ok(&run(s, sm, "all")), RunValue::Signed(111));
            assert_eq!(ok(&run(s, sm, "call_all")), RunValue::Signed(111));
            // A void entry: `static_assert((vcheck(), true))`.
            assert_eq!(ok(&run(s, sm, "vcheck")), RunValue::Void);
            // Not a consteval block's expression ([expr.const.core]/2.32).
            assert_eq!(ok(&run(s, sm, "plainly")), RunValue::Signed(0));
            assert_eq!(ok(&run(s, sm, "call_plainly")), RunValue::Signed(0));
        }
    });
}

const CALLS: &str = "consteval int sq(int x) { return x * x; }\n\
constexpr int uses() { return sq(12) + 1; }\n\
consteval int cuses() { return sq(12) + 1; }\n\
template <class T> constexpr int tuses() { return sq(sizeof(T) * 3) + 1; }\n\
using Later = decltype(tuses<int>());\n\
consteval int id(int x) { return x; }\n\
template <class T> constexpr int esc() { T n = sizeof(T); return id(n) + 1; }\n\
constexpr int at(int i) { int a[2]{}; return a[i]; }\n\
template <class T> constexpr int esc_bad() { T n = sizeof(T); return id(n) + at(5); }\n\
consteval int use_esc() { return esc<int>() + esc_bad<int>(); }\n\
using Unevaluated = decltype(esc<long>() + esc_bad<long>());\n";

#[test]
fn consteval_calls_in_a_constexpr_entry() {
    session(CALLS, |s, sm| {
        // `sq(12)` in a constexpr entry is an immediate invocation
        // evaluated with the translation unit ([expr.const.imm]/1): the
        // run reads its value. In a consteval entry it is part of the run.
        let ce = run(s, sm, "uses");
        let cv = run(s, sm, "cuses");
        assert_eq!(ok(&ce), RunValue::Signed(145));
        assert_eq!(ok(&cv), RunValue::Signed(145));
        assert_eq!((ce.calls, ce.nested_steps), (1, 0), "{ce:?}");
        assert_eq!(cv.calls, 2, "{cv:?}");
        assert!(cv.steps > ce.steps, "{cv:?} {ce:?}");
        // A specialization the run instantiates: its immediate invocation
        // is a nested evaluation of the instantiation, charged to every
        // run that reaches it (§5.1), on a fresh and a used session.
        let fresh = session(CALLS, |s, sm| {
            let e = s.entry(spec_of(s, "tuses", 0)).unwrap();
            seen(sm, &s.run(e, &mut Give(0), RunBudget::PROGRAM, &CancelToken::new()))
        });
        assert_eq!(fresh.result, Ok(RunValue::Signed(145)));
        assert!(fresh.nested_steps > 0 && fresh.calls == 1, "{fresh:?}");
        let e = s.entry(spec_of(s, "tuses", 0)).unwrap();
        for _ in 0..3 {
            let r = seen(sm, &s.run(e, &mut Give(0), RunBudget::PROGRAM, &CancelToken::new()));
            assert_eq!(r, fresh);
        }
    });
}

#[test]
fn an_escalated_function_is_an_immediate_entry() {
    // `esc<int>` is immediate ([expr.const.imm]/2.2, 4.2): a run of it is
    // an immediate invocation, reported as clang reports one.
    session(CALLS, |s, sm| {
        let good = s.entry(spec_of(s, "esc", 0)).expect("an escalated entry");
        let bad = s.entry(spec_of(s, "esc_bad", 0)).expect("an escalated entry");
        for _ in 0..2 {
            let r = seen(sm, &s.run(good, &mut Give(0), RunBudget::PROGRAM, &CancelToken::new()));
            assert_eq!(ok(&r), RunValue::Signed(5));
            let r = seen(sm, &s.run(bad, &mut Give(0), RunBudget::PROGRAM, &CancelToken::new()));
            assert_eq!(r.result, Err(RunError::Failed));
            assert_eq!(
                r.diags,
                [
                    "13:34 call to immediate function 'esc_bad<int>' is not a constant expression",
                    "13:70 'esc_bad<int>' is an immediate function because its body contains a \
                     call to a consteval function 'id' and that call is not a constant expression",
                    "12:46 cannot refer to element 5 of array of 2 elements in a constant expression",
                    "13:78 in call to 'at(5)'",
                    "13:34 in call to 'esc_bad<int>()'",
                ],
            );
        }
        // Specializations the first run instantiates: they escalate in it
        // (constexpr entries when resolved, immediate when reported).
        let good = s.entry(spec_of(s, "esc", 1)).expect("a constexpr entry");
        let bad = s.entry(spec_of(s, "esc_bad", 1)).expect("a constexpr entry");
        for _ in 0..2 {
            let r = seen(sm, &s.run(good, &mut Give(0), RunBudget::PROGRAM, &CancelToken::new()));
            assert_eq!(ok(&r), RunValue::Signed(9));
            let r = seen(sm, &s.run(bad, &mut Give(0), RunBudget::PROGRAM, &CancelToken::new()));
            assert_eq!(r.result, Err(RunError::Failed));
            assert_eq!(
                r.diags[..2],
                [
                    "13:34 call to immediate function 'esc_bad<long>' is not a constant expression",
                    "13:70 'esc_bad<long>' is an immediate function because its body contains a \
                     call to a consteval function 'id' and that call is not a constant expression",
                ],
            );
        }
    });
}

const FAILS: &str = "int puts(const char*);\n\
constexpr int at(int i) { int a[2]{}; return a[i]; }\n\
constexpr int ub() { return at(5); }\n\
constexpr int nc() { puts(\"x\"); return 1; }\n\
constexpr int leak() { new int(3); return 1; }\n\
constexpr void vub() { at(5); }\n\
constexpr int thrown() { throw 7; return 0; }\n\
constexpr int depends() { return hc(1) > 5 ? at(5) : 1; }\n\
constexpr int good() { int s = 0; for (int i = 0; i < 10; ++i) s += i; return s; }\n";

#[test]
fn failures_have_the_constexpr_variable_notes() {
    // The notes clang 22 and 19 give for `constexpr int v = ub();` (and
    // the others), but for the outermost `in call to`, which is at the
    // entry: a run has no call expression.
    session(FAILS, |s, sm| {
        let cases: [(&str, &[&str]); 4] = [
            (
                "ub",
                &[
                    "7:15 call to constexpr function 'ub' is not a constant expression",
                    "6:46 cannot refer to element 5 of array of 2 elements in a constant expression",
                    "7:29 in call to 'at(5)'",
                    "7:15 in call to 'ub()'",
                ],
            ),
            (
                "nc",
                &[
                    "8:15 call to constexpr function 'nc' is not a constant expression",
                    "8:22 non-constexpr function 'puts' cannot be used in a constant expression",
                    "8:15 in call to 'nc()'",
                    "5:5 declared here",
                ],
            ),
            (
                "leak",
                &[
                    "9:15 call to constexpr function 'leak' is not a constant expression",
                    "9:24 allocation performed here was not deallocated",
                ],
            ),
            (
                "vub",
                &[
                    "10:16 call to constexpr function 'vub' is not a constant expression",
                    "6:46 cannot refer to element 5 of array of 2 elements in a constant expression",
                    "10:24 in call to 'at(5)'",
                    "10:16 in call to 'vub()'",
                ],
            ),
        ];
        for _ in 0..2 {
            for (name, want) in cases {
                let r = run(s, sm, name);
                assert_eq!(r.result, Err(RunError::Failed), "{name}");
                assert_eq!(r.diags, want, "{name}");
                assert_eq!(ok(&run(s, sm, "good")), RunValue::Signed(45));
            }
            let r = run(s, sm, "thrown");
            assert_eq!(
                r.result,
                Err(RunError::Uncaught {
                    type_name: "int".into(),
                    what: None,
                    scalars: Vec::new()
                })
            );
            // Constant for one run, not for another.
            assert_eq!(ok(&run_with(s, sm, "depends", 3)), RunValue::Signed(1));
            let r = run_with(s, sm, "depends", 9);
            assert_eq!(r.result, Err(RunError::Failed));
            assert!(r.diags[0].contains("call to constexpr function 'depends'"), "{:?}", r.diags);
            assert_eq!(ok(&run_with(s, sm, "depends", 3)), RunValue::Signed(1));
        }
    });
}

const WRAP: &str = "constexpr int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }\n\
constexpr int at(int i) { int a[2]{}; return a[i]; }\n\
constexpr int body() { int s = 0; for (int i = 0; i < 20; ++i) s += fib(i % 8); return s; }\n\
consteval int wrapped() { return body(); }\n\
constexpr int other() { return fib(15); }\n\
consteval int wrapped_other() { return other(); }\n\
constexpr int bad() { return at(5); }\n\
consteval int wrapped_bad() { return bad(); }\n";

#[test]
fn a_consteval_wrapper_differs_by_the_wrapper_only() {
    session(WRAP, |s, sm| {
        let mut extra = None;
        for (f, w) in [("body", "wrapped"), ("other", "wrapped_other")] {
            for _ in 0..2 {
                let a = run(s, sm, f);
                let b = run(s, sm, w);
                assert_eq!(ok(&a), ok(&b), "{f}");
                assert_eq!((a.nested_steps, a.nested_calls), (b.nested_steps, b.nested_calls));
                assert_eq!(b.calls, a.calls + 1, "{f}");
                // The wrapper's own steps: the same for every body.
                let k = b.steps - a.steps;
                assert_eq!(*extra.get_or_insert(k), k, "{f}");
            }
        }
        // A failure: the wrapper's headline and one more call note.
        let a = run(s, sm, "bad");
        let b = run(s, sm, "wrapped_bad");
        assert_eq!((&a.result, &b.result), (&Err(RunError::Failed), &Err(RunError::Failed)));
        assert_eq!(
            a.diags[0],
            "11:15 call to constexpr function 'bad' is not a constant expression"
        );
        assert_eq!(
            b.diags[0],
            "12:15 call to consteval function 'wrapped_bad' is not a constant expression"
        );
        assert_eq!(a.diags[1..a.diags.len() - 1], b.diags[1..a.diags.len() - 1]);
        assert_eq!(b.diags.len(), a.diags.len() + 1, "{:?}", b.diags);
    });
}

const MIX: &str = "[[deprecated(\"use new\")]] constexpr int old(int x) { return x; }\n\
constexpr int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }\n\
template <class T> struct W {\n\
  static constexpr int k = sizeof(T) + fib(10);\n\
  static constexpr int helper() { return old(k + T{}) + fib(12); }\n\
  static constexpr int a() { return helper() + fib(14); }\n\
  static consteval int b() { return helper() + 1; }\n\
  static constexpr int c() { return a() + (hc(1) > 2 ? 1 : 0); }\n\
};\n\
static_assert(sizeof(W<long>) > 0);\n\
using X = W<long>;\n";

#[test]
fn constexpr_runs_are_deterministic() {
    // Run 1 and run N, fresh and used sessions, either order, with an
    // instantiation (and its warning) done by whichever run comes first.
    let fresh = |name: &str| session(MIX, |s, sm| run_with(s, sm, name, 5));
    let names = ["X::a", "X::b", "X::c"];
    let want: Vec<Seen> = names.iter().map(|n| fresh(n)).collect();
    for w in &want {
        assert!(w.result.is_ok(), "{w:?}");
        assert!(w.diags.iter().any(|d| d.contains("'old' is deprecated")), "{w:?}");
    }
    for order in [[0, 1, 2], [2, 1, 0], [1, 0, 2]] {
        session(MIX, |s, sm| {
            for _ in 0..3 {
                for i in order {
                    let name = names[i];
                    assert_eq!(run_with(s, sm, name, 5), want[i], "{name} in {order:?}");
                }
            }
        });
    }
}
