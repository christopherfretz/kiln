//! WP-2071: the results of evaluation-session entries that return class
//! types and other non-integral values (spec/eval-session.md §4.1),
//! through the library interface a tooling client uses: every kind of
//! value, pointers and their designators, the constants table and its
//! identity rules, the `decl` rule, the size cap, determinism over runs
//! and sessions, refused types and values, and `variable_value`.
#![forbid(unsafe_code)]

use std::sync::Arc;

use kiln_diag::{DiagEngine, DiagOptions, Diagnostic, MemorySink};
use kiln_parse::{
    CancelToken, CharUnit, ConstexprLimits, EntryError, EvalSession, FieldKind, FloatKind,
    HostCalls, HostError, HostOut, InfoKind, IntValue, ParseOptions, PathStep, PointerBase,
    RunBudget, RunError, RunReport, RunValue, ValueError, ValueTree, parse_preprocessed_retained,
};
use kiln_pp::{ClangFeatures, ModuleOptions, PpOptions, Preprocessor};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// The `std::meta` declarations the tests use, declared directly so they
/// do not depend on a library header.
const PRE: &str = "namespace std::meta {\n\
using info = decltype(^^int);\n\
template <class T> consteval T extract(info r);\n\
template <class T> consteval info reflect_constant(const T& v);\n\
template <class R> consteval info substitute(info templ, R&& args);\n\
template <class R> consteval info __kiln_reflect_constant_string(R&& r);\n\
template <class R> consteval info __kiln_reflect_constant_array(R&& r);\n\
}\n\
using info = std::meta::info;\n";

/// Every handler call fails: no test makes one.
struct NoHost;

impl HostCalls for NoHost {
    fn call(
        &mut self,
        _op: u32,
        _ints: &[u64],
        _bytes: &[u8],
        _out: &mut HostOut<'_>,
    ) -> Result<u64, HostError> {
        Err(HostError::new("no handler"))
    }
}

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

/// Parses `PRE` + `src` with `-freflection`, opens a session on it and
/// calls `f` with it.
fn session<R>(src: &str, f: impl for<'a, 'sm> FnOnce(&mut EvalSession<'a, 'sm>) -> R) -> R {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", format!("{PRE}{src}")).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let opts = ParseOptions { reflection: true, ..ParseOptions::default() };
    let dopts = DiagOptions { clang_compat: opts.clang_compat, ..DiagOptions::default() };
    let engine = DiagEngine::new(&sm, &dopts, sink.clone());
    let mut ppo = PpOptions::new(opts.std);
    ppo.parser_pragmas = true;
    ppo.reflection = true;
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
        None,
    );
    let tu = render(&sm, &sink.take());
    assert!(tu.is_empty(), "translation unit diagnostics:\n{}", tu.join("\n"));
    let mut s = EvalSession::open(sema).expect("session opens");
    let r = f(&mut s);
    let late = render(&sm, &sink.take());
    assert!(late.is_empty(), "runs' diagnostics reached the sink:\n{}", late.join("\n"));
    r
}

fn run_budget(s: &mut EvalSession<'_, '_>, name: &str, budget: RunBudget) -> RunReport {
    let e = s.entry_named(name).unwrap_or_else(|e| panic!("entry {name}: {e}"));
    s.run(e, &mut NoHost, budget, &CancelToken::new())
}

/// Runs `name`; its value and constants table, which must not fail.
#[track_caller]
fn tree(s: &mut EvalSession<'_, '_>, name: &str) -> ValueTree {
    let r = run_budget(s, name, RunBudget::PROGRAM);
    let msgs: Vec<&str> = r.diagnostics.iter().map(|d| d.message.as_str()).collect();
    assert!(r.diagnostics.is_empty(), "{name}: unexpected diagnostics: {msgs:?}");
    match r.result {
        Ok(root) => ValueTree { root, constants: r.constants },
        Err(e) => panic!("{name}: run failed: {e:?}"),
    }
}

#[track_caller]
fn val(s: &mut EvalSession<'_, '_>, name: &str) -> RunValue {
    tree(s, name).root
}

fn record(v: &RunValue) -> &kiln_parse::RecordValue {
    match v {
        RunValue::Record(r) => r,
        other => panic!("not a record: {other:?}"),
    }
}

fn pointer(v: &RunValue) -> &kiln_parse::PointerValue {
    match v {
        RunValue::Pointer(p) => p,
        other => panic!("not a pointer: {other:?}"),
    }
}

fn names(v: &RunValue) -> Vec<(FieldKind, String)> {
    record(v).fields.iter().map(|f| (f.kind, f.name.to_string())).collect()
}

fn member(n: &str) -> PathStep {
    PathStep::Member(n.into())
}

#[test]
fn scalar_values() {
    session(
        "consteval bool b() { return true; }\n\
         consteval char c() { return 'x'; }\n\
         consteval unsigned u() { return 7u; }\n\
         consteval __int128 big() { return -((__int128)1 << 100); }\n\
         consteval _BitInt(100) bi() { return -3; }\n\
         consteval unsigned _BitInt(128) ubi() { return ~(unsigned _BitInt(128))0; }\n\
         consteval float f() { return 1.5f; }\n\
         consteval double negzero() { return -0.0; }\n\
         consteval double nan() { return -__builtin_nan(\"0x123\"); }\n\
         consteval long double ld() { return 1.0L; }\n\
         consteval _Complex double cx() { return 1.0 + 2.0i; }\n\
         consteval int* null() { return nullptr; }\n\
         consteval decltype(nullptr) np() { return nullptr; }\n\
         struct S { int a; };\n\
         consteval int S::* nullmp() { return nullptr; }\n",
        |s| {
            assert_eq!(val(s, "b"), RunValue::Bool(true));
            assert_eq!(val(s, "c"), RunValue::Signed(i128::from(b'x')));
            assert_eq!(val(s, "u"), RunValue::Unsigned(7));
            assert_eq!(val(s, "big"), RunValue::Signed(-(1i128 << 100)));
            assert_eq!(val(s, "bi"), RunValue::Signed(-3));
            assert_eq!(val(s, "ubi"), RunValue::Unsigned(u128::MAX));
            let f =
                |v: f32| RunValue::Float { kind: FloatKind::Single, bits: u128::from(v.to_bits()) };
            assert_eq!(val(s, "f"), f(1.5));
            let d = |b: u64| RunValue::Float { kind: FloatKind::Double, bits: u128::from(b) };
            assert_eq!(val(s, "negzero"), d((-0.0f64).to_bits()));
            // The sign and the payload survive.
            assert_eq!(val(s, "nan"), d(0xfff8_0000_0000_0123));
            // x87: 1.0 is the explicit integer bit, exponent 0x3fff.
            assert_eq!(
                val(s, "ld"),
                RunValue::Float {
                    kind: FloatKind::X87Extended,
                    bits: (0x3fffu128 << 64) | (1 << 63)
                }
            );
            assert_eq!(
                val(s, "cx"),
                RunValue::Array(Box::new([d(1.0f64.to_bits()), d(2.0f64.to_bits())]))
            );
            assert_eq!(val(s, "null"), RunValue::Null);
            assert_eq!(val(s, "np"), RunValue::Null);
            assert_eq!(val(s, "nullmp"), RunValue::Null);
        },
    );
}

#[test]
fn enums_with_and_without_an_enumerator() {
    session(
        "enum class E : short { A = -1, B = 2, C = 2 };\n\
         enum U : unsigned char { X = 200 };\n\
         consteval E named() { return E::C; }\n\
         consteval E unnamed() { return (E)7; }\n\
         consteval E neg() { return E::A; }\n\
         consteval U u() { return X; }\n",
        |s| {
            let RunValue::Enum(e) = val(s, "named") else { panic!("not an enum") };
            assert_eq!(&*e.ty.display, "E");
            assert!(e.ty.decl.is_some());
            assert_eq!(e.value, IntValue::Signed(2));
            // The first enumerator declared with the value.
            assert_eq!(e.enumerator.as_deref(), Some("B"));
            let RunValue::Enum(e) = val(s, "unnamed") else { panic!("not an enum") };
            assert_eq!((e.value, e.enumerator), (IntValue::Signed(7), None));
            let RunValue::Enum(e) = val(s, "neg") else { panic!("not an enum") };
            assert_eq!((e.value, e.enumerator.as_deref()), (IntValue::Signed(-1), Some("A")));
            let RunValue::Enum(e) = val(s, "u") else { panic!("not an enum") };
            assert_eq!((e.value, e.enumerator.as_deref()), (IntValue::Unsigned(200), Some("X")));
        },
    );
}

#[test]
fn classes_bases_bit_fields_and_arrays() {
    session(
        "struct Empty {};\n\
         struct Base { int b = 1; };\n\
         struct In { short x; char tag[3]; };\n\
         struct D : Empty, Base {\n\
           unsigned flag : 1;\n\
           int : 3;\n\
           int sbits : 4;\n\
           In in[2];\n\
           union { int i; float f; };\n\
           char16_t w[2];\n\
           Empty em;\n\
           [[no_unique_address]] Empty nua;\n\
         };\n\
         consteval D d() {\n\
           D v{};\n\
           v.flag = 1; v.sbits = -2;\n\
           v.in[1].x = 5; v.in[1].tag[0] = 'q';\n\
           v.f = 2.5f; v.w[0] = u'\\u00e9';\n\
           return v;\n\
         }\n\
         union V { int i; double d; };\n\
         consteval V u() { V v{}; v.d = 0.5; return v; }\n",
        |s| {
            let v = val(s, "d");
            let r = record(&v);
            assert_eq!(&*r.ty.display, "D");
            assert!(!r.union);
            // Bases then members in declaration order; the empty base is
            // there and the unnamed bit-field is not.
            assert_eq!(
                names(&v),
                vec![
                    (FieldKind::Base, "Empty".into()),
                    (FieldKind::Base, "Base".into()),
                    (FieldKind::Member, "flag".into()),
                    (FieldKind::Member, "sbits".into()),
                    (FieldKind::Member, "in".into()),
                    (FieldKind::Anonymous, String::new()),
                    (FieldKind::Member, "w".into()),
                    (FieldKind::Member, "em".into()),
                    (FieldKind::Member, "nua".into()),
                ]
            );
            for empty in ["Empty", "em", "nua"] {
                let e = v.field(empty).unwrap();
                assert!(names(e).is_empty() && &*record(e).ty.display == "Empty", "{empty}");
            }
            assert_eq!(v.field("Base").unwrap().field("b"), Some(&RunValue::Signed(1)));
            assert_eq!(v.field("flag"), Some(&RunValue::Unsigned(1)));
            assert_eq!(v.field("sbits"), Some(&RunValue::Signed(-2)));
            let RunValue::Array(ins) = v.field("in").unwrap() else { panic!("in") };
            assert_eq!(ins.len(), 2);
            assert_eq!(ins[1].field("x"), Some(&RunValue::Signed(5)));
            assert_eq!(
                ins[1].field("tag"),
                Some(&RunValue::Text { unit: CharUnit::Char, data: Box::new(*b"q\0\0") })
            );
            let anon = &record(&v).fields[5].value;
            assert!(record(anon).union);
            assert_eq!(
                names(anon),
                vec![(FieldKind::Member, "f".into())],
                "the active member only"
            );
            assert_eq!(
                v.field("w").unwrap().text_units(),
                Some(vec![0xe9, 0]),
                "char16_t units, little-endian"
            );
            let u = val(s, "u");
            assert!(record(&u).union);
            assert_eq!(
                u.field("d"),
                Some(&RunValue::Float {
                    kind: FloatKind::Double,
                    bits: u128::from(0.5f64.to_bits())
                })
            );
            assert_eq!(record(&u).fields.len(), 1);
        },
    );
}

#[test]
fn pointers_designators_and_member_pointers() {
    session(
        "struct S { int a; int b[3]; };\n\
         constexpr S s{1, {2, 3, 4}};\n\
         constexpr int g[4] = {};\n\
         constexpr int x = 0;\n\
         int f() { return 0; }\n\
         struct Ptrs { const int* elem; const int* end; const int* past; const int* sub; \
           const S* whole; int (*fn)(); int S::* mp; const int& ref; const float* un; \
           const int* apast; const void* vp; };\n\
         union U { int a; float b; };\n\
         struct WU { int pad; U u; };\n\
         constexpr WU wu{0, {.b = 1.0f}};\n\
         consteval Ptrs p() { return {&g[2], &g[4], &x + 1, &s.b[1], &s, &f, &S::a, s.a, \
           &wu.u.b, &s.a + 1, &s.b[2]}; }\n",
        |s| {
            let v = val(s, "p");
            let var = |p: &kiln_parse::PointerValue| match &p.base {
                PointerBase::Variable(e) => e.display.to_string(),
                other => panic!("not a variable: {other:?}"),
            };
            let elem = pointer(v.field("elem").unwrap());
            assert_eq!(
                (var(elem).as_str(), &*elem.path, elem.past_end),
                ("g", &[PathStep::Index(2)][..], false)
            );
            // `&g[N]` is the index N, not past the end of an element.
            let end = pointer(v.field("end").unwrap());
            assert_eq!((&*end.path, end.past_end), (&[PathStep::Index(4)][..], false));
            // `&x + 1` for a scalar `x`: one past the end of `x`.
            let past = pointer(v.field("past").unwrap());
            assert_eq!((var(past).as_str(), &*past.path, past.past_end), ("x", &[][..], true));
            let sub = pointer(v.field("sub").unwrap());
            assert_eq!(&*sub.path, &[member("b"), PathStep::Index(1)][..]);
            assert!(matches!(&sub.base, PointerBase::Variable(e) if e.decl.is_some()));
            let whole = pointer(v.field("whole").unwrap());
            assert_eq!((var(whole).as_str(), &*whole.path), ("s", &[][..]));
            let fnp = pointer(v.field("fn").unwrap());
            assert!(
                matches!(&fnp.base, PointerBase::Function(e) if e.display.contains('f') && e.decl.is_some())
            );
            let RunValue::MemberPointer(m) = v.field("mp").unwrap() else { panic!("mp") };
            assert!(m.member.display.ends_with('a'), "{}", m.member.display);
            assert!(m.member.decl.is_some());
            assert!(!m.derived && m.path.is_empty());
            // A reference member reads as a pointer to what it binds.
            let r = pointer(v.field("ref").unwrap());
            assert_eq!((var(r).as_str(), &*r.path), ("s", &[member("a")][..]));
            // Into a union: the member of the pointee's type.
            let un = pointer(v.field("un").unwrap());
            assert_eq!((var(un).as_str(), &*un.path), ("wu", &[member("u"), member("b")][..]));
            // One past the end of a member that is no array element.
            let apast = pointer(v.field("apast").unwrap());
            assert_eq!((&*apast.path, apast.past_end), (&[member("a")][..], true));
            let vp = pointer(v.field("vp").unwrap());
            assert_eq!((&*vp.path, vp.past_end), (&[member("b"), PathStep::Index(2)][..], false));
        },
    );
}

#[test]
fn pointers_into_unions_name_the_member() {
    // Members of one union that share an offset and a type: the path names
    // the member the pointer designates (its union path, spec/ast.md §9.1),
    // through nested unions and array elements too.
    session(
        "union U { int a; int b; };\n\
         union In { int p; int q; };\n\
         union Out { In i; In j; };\n\
         struct H { int pad; U arr[2]; Out o; };\n\
         constexpr U u{.b = 1};\n\
         constexpr H h{};\n\
         struct Ptrs { const int* a; const int* b; const int* nest; const int* elem; \
           const int* dflt; };\n\
         consteval Ptrs p() { return {&u.a, &u.b, &h.o.j.q, &h.arr[1].b, &h.o.i.p}; }\n\
         constexpr Ptrs tu = p();\n\
         constexpr const int* pb = &u.b;\n",
        |s| {
            let run = tree(s, "p");
            let path = |v: &RunValue, f: &str| pointer(v.field(f).unwrap()).path.to_vec();
            assert_eq!(path(&run.root, "a"), [member("a")]);
            assert_eq!(path(&run.root, "b"), [member("b")]);
            assert_eq!(path(&run.root, "nest"), [member("o"), member("j"), member("q")]);
            assert_eq!(path(&run.root, "elem"), [member("arr"), PathStep::Index(1), member("b")]);
            assert_eq!(path(&run.root, "dflt"), [member("o"), member("i"), member("p")]);
            // The translation unit's own constants say the same.
            let cap = RunBudget::PROGRAM.result_bytes;
            assert_eq!(s.variable_value_named("tu", cap), Ok(run));
            let pb = s.variable_value_named("pb", cap).expect("pb");
            assert_eq!(&*pointer(&pb.root).path, &[member("b")][..]);
        },
    );
}

fn vnames(v: &RunValue) -> Vec<(FieldKind, String)> {
    record(v).virtual_bases.iter().map(|f| (f.kind, f.name.to_string())).collect()
}

fn vbase<'v>(v: &'v RunValue, name: &str) -> &'v RunValue {
    let r = record(v);
    match r.virtual_bases.iter().find(|f| &*f.name == name) {
        Some(f) => &f.value,
        None => panic!("no virtual base {name} in {r:?}"),
    }
}

fn base(n: &str) -> PathStep {
    PathStep::Base(n.into())
}

fn vstep(n: &str) -> PathStep {
    PathStep::VirtualBase(n.into())
}

/// Runs `entry` twice and checks both runs against the translation
/// unit's own constant `var` (spec/eval-session.md §4.1 "A variable's
/// value"); the tree.
#[track_caller]
fn same_as_tu(s: &mut EvalSession<'_, '_>, entry: &str, var: &str) -> ValueTree {
    let cap = RunBudget::PROGRAM.result_bytes;
    let first = tree(s, entry);
    for _ in 0..3 {
        assert_eq!(tree(s, entry), first, "{entry}: run N");
    }
    assert_eq!(s.variable_value_named(var, cap), Ok(first.clone()), "{entry} vs {var}");
    first
}

#[test]
fn classes_with_virtual_bases() {
    // [class.mi], [class.base.init]/13.1 (WP-2210): each virtual base
    // subobject once, at the most derived object, in initialization order.
    session(
        "struct V { int v; constexpr V(int x = 1) : v(x) {} };\n\
         struct W { int w = 2; };\n\
         struct N { int n = 3; };\n\
         struct One : virtual V { int a; constexpr One() : V(10), a(11) {} };\n\
         struct L : virtual V { int l = 4; };\n\
         struct Rt : virtual V { int r = 5; };\n\
         struct Dia : L, Rt { int d; constexpr Dia() : V(20), d(6) {} };\n\
         struct X : virtual W { int x = 7; };\n\
         struct Mix : N, X, virtual V { int m = 8; };\n\
         struct VV : virtual W { int q = 9; };\n\
         struct Deep : virtual VV, N {};\n\
         struct Holder { int h = 12; One o; };\n\
         consteval One one() { return One{}; }\n\
         consteval Dia dia() { return Dia{}; }\n\
         consteval Mix mix() { return Mix{}; }\n\
         consteval Deep deep() { return Deep{}; }\n\
         consteval Holder holder() { return Holder{}; }\n\
         constexpr One one_v = one();\n\
         constexpr Dia dia_v = dia();\n\
         constexpr Mix mix_v = mix();\n\
         constexpr Deep deep_v = deep();\n\
         constexpr Holder holder_v = holder();\n",
        |s| {
            let k = |n: &str| (FieldKind::Base, n.to_owned());
            let m = |n: &str| (FieldKind::Member, n.to_owned());
            // One virtual base.
            let one = same_as_tu(s, "one", "one_v").root;
            assert_eq!(names(&one), [m("a")]);
            assert_eq!(vnames(&one), [k("V")]);
            assert_eq!(one.field("a"), Some(&RunValue::Signed(11)));
            assert_eq!(vbase(&one, "V").field("v"), Some(&RunValue::Signed(10)));
            assert!(record(vbase(&one, "V")).virtual_bases.is_empty());
            // A diamond: the shared virtual base once, not in the bases.
            let dia = same_as_tu(s, "dia", "dia_v").root;
            assert_eq!(names(&dia), [k("L"), k("Rt"), m("d")]);
            assert_eq!(vnames(&dia), [k("V")]);
            assert_eq!(vbase(&dia, "V").field("v"), Some(&RunValue::Signed(20)));
            let l = dia.field("L").unwrap();
            assert_eq!(names(l), [m("l")]);
            assert!(record(l).virtual_bases.is_empty(), "a base subobject holds none");
            assert_eq!(l.field("l"), Some(&RunValue::Signed(4)));
            assert_eq!(dia.field("Rt").unwrap().field("r"), Some(&RunValue::Signed(5)));
            assert_eq!(dia.field("d"), Some(&RunValue::Signed(6)));
            // Beside non-virtual bases: each direct base's virtual bases,
            // then the base itself if virtual.
            let mix = same_as_tu(s, "mix", "mix_v").root;
            assert_eq!(names(&mix), [k("N"), k("X"), m("m")]);
            assert_eq!(vnames(&mix), [k("W"), k("V")]);
            assert_eq!(vbase(&mix, "W").field("w"), Some(&RunValue::Signed(2)));
            assert_eq!(vbase(&mix, "V").field("v"), Some(&RunValue::Signed(1)));
            assert_eq!(mix.field("X").unwrap().field("x"), Some(&RunValue::Signed(7)));
            // A virtual base of a virtual base comes first.
            let deep = same_as_tu(s, "deep", "deep_v").root;
            assert_eq!(names(&deep), [k("N")]);
            assert_eq!(vnames(&deep), [k("W"), k("VV")]);
            assert_eq!(vbase(&deep, "VV").field("q"), Some(&RunValue::Signed(9)));
            assert!(record(vbase(&deep, "VV")).virtual_bases.is_empty());
            // A member is a most derived object: its virtual bases are its
            // own.
            let holder = same_as_tu(s, "holder", "holder_v").root;
            assert!(record(&holder).virtual_bases.is_empty());
            let o = holder.field("o").unwrap();
            assert_eq!(vnames(o), [k("V")]);
            assert_eq!(vbase(o, "V").field("v"), Some(&RunValue::Signed(10)));
            // A hand-written tree of the simplest.
            let int = |v: i128| RunValue::Signed(v);
            let field =
                |kind, name: &str, value| kiln_parse::Field { kind, name: name.into(), value };
            let rec = |ty: &str, fields: Vec<kiln_parse::Field>, vb: Vec<kiln_parse::Field>| {
                RunValue::Record(Box::new(kiln_parse::RecordValue {
                    ty: kiln_parse::Entity { display: ty.into(), decl: None },
                    union: false,
                    fields: fields.into_boxed_slice(),
                    virtual_bases: vb.into_boxed_slice(),
                }))
            };
            let mut want = rec(
                "One",
                vec![field(FieldKind::Member, "a", int(11))],
                vec![field(
                    FieldKind::Base,
                    "V",
                    rec("V", vec![field(FieldKind::Member, "v", int(10))], vec![]),
                )],
            );
            // `decl` is the translation unit's: copy it over.
            let decl_of = |v: &RunValue| record(v).ty.decl;
            if let RunValue::Record(r) = &mut want {
                r.ty.decl = decl_of(&one);
                if let RunValue::Record(vr) = &mut r.virtual_bases[0].value {
                    vr.ty.decl = decl_of(vbase(&one, "V"));
                }
            }
            assert!(decl_of(&one).is_some());
            assert_eq!(one, want);
        },
    );
}

#[test]
fn pointers_into_virtual_bases() {
    // A pointer into a virtual base subobject names it (WP-2210).
    session(
        "struct V { int v = 1; int arr[2] = {2, 3}; };\n\
         struct L : virtual V { int l = 4; };\n\
         struct Rt : virtual V { int r = 5; };\n\
         struct Dia : L, Rt { int d = 6; };\n\
         struct Holder { int h = 7; Dia in; };\n\
         constexpr Dia dia{};\n\
         constexpr Holder holder{};\n\
         struct Ptrs { const int* v; const V* whole; const int* l; const L* lb; \
           const int* elem; const int* nested; const V* through; const int* past; };\n\
         consteval Ptrs p() { return {&dia.v, &dia, &dia.l, &dia, &dia.arr[1], \
           &holder.in.v, static_cast<const L*>(&dia), &dia.v + 1}; }\n\
         constexpr Ptrs tu = p();\n\
         constexpr const int* pv = &dia.v;\n",
        |s| {
            let run = same_as_tu(s, "p", "tu");
            let path = |f: &str| pointer(run.root.field(f).unwrap()).path.to_vec();
            assert_eq!(path("v"), [vstep("V"), member("v")]);
            assert_eq!(path("whole"), [vstep("V")]);
            assert_eq!(path("l"), [base("L"), member("l")]);
            assert_eq!(path("lb"), [base("L")]);
            assert_eq!(path("elem"), [vstep("V"), member("arr"), PathStep::Index(1)]);
            assert_eq!(path("nested"), [member("in"), vstep("V"), member("v")]);
            // `L`'s virtual base is the most derived object's.
            assert_eq!(path("through"), [vstep("V")]);
            let past = pointer(run.root.field("past").unwrap());
            assert_eq!((&*past.path, past.past_end), (&[vstep("V"), member("v")][..], true));
            for f in ["v", "whole", "l", "elem", "past"] {
                let p = pointer(run.root.field(f).unwrap());
                assert!(
                    matches!(&p.base, PointerBase::Variable(e) if &*e.display == "dia"),
                    "{f}: {p:?}"
                );
            }
            let cap = RunBudget::PROGRAM.result_bytes;
            let pv = s.variable_value_named("pv", cap).expect("pv");
            assert_eq!(&*pointer(&pv.root).path, &[vstep("V"), member("v")][..]);
        },
    );
}

#[test]
fn member_pointers_give_their_conversion_path() {
    // [conv.mem]/2, [expr.static.cast]/12, [expr.eq]/4 (WP-2212): the class
    // chain of clang's `APValue` member pointer path.
    session(
        "struct B { int x; int y; constexpr int get() const { return x; } };\n\
         struct D : B { int z; };\n\
         struct E : D {};\n\
         struct MPs { int B::* bx; int D::* dx; int E::* ex; int B::* fromd; int D::* null; \
           int (B::*bf)() const; int (D::*df)() const; int D::* dz; int B::* back; };\n\
         consteval MPs mps() { return {&B::x, &B::x, &B::x, static_cast<int B::*>(&D::z), \
           nullptr, &B::get, &B::get, &D::z, \
           static_cast<int B::*>(static_cast<int D::*>(&B::x))}; }\n\
         constexpr MPs tu = mps();\n\
         constexpr int D::* dx_v = &B::x;\n",
        |s| {
            let run = same_as_tu(s, "mps", "tu");
            let mp = |f: &str| match run.root.field(f).unwrap() {
                RunValue::MemberPointer(m) => m.as_ref().clone(),
                other => panic!("{f}: not a member pointer: {other:?}"),
            };
            let shape = |m: &kiln_parse::MemberPointerValue| {
                let path: Vec<String> = m.path.iter().map(|e| e.display.to_string()).collect();
                (m.member.display.to_string(), m.derived, path)
            };
            let none: Vec<String> = Vec::new();
            let bx = mp("bx");
            assert!(bx.member.display.ends_with('x'), "{}", bx.member.display);
            assert!(!bx.derived);
            assert_eq!(shape(&bx).2, none);
            // The same member as `int D::*` and `int E::*`: the path tells
            // them apart.
            let dx = mp("dx");
            assert_eq!(dx.member, bx.member);
            assert_eq!((dx.derived, shape(&dx).2), (false, vec!["D".to_owned()]));
            assert!(dx.path[0].decl.is_some());
            assert_ne!(dx, bx);
            let ex = mp("ex");
            assert_eq!((ex.derived, shape(&ex).2), (false, vec!["D".to_owned(), "E".to_owned()]));
            // A derived class's member cast to the base: towards bases.
            let fromd = mp("fromd");
            assert!(fromd.member.display.ends_with('z'), "{}", fromd.member.display);
            assert_eq!((fromd.derived, shape(&fromd).2), (true, vec!["B".to_owned()]));
            assert_eq!(run.root.field("null"), Some(&RunValue::Null));
            // A member function through a base.
            let bf = mp("bf");
            assert!(bf.member.display.contains("get"), "{}", bf.member.display);
            assert_eq!((bf.derived, shape(&bf).2), (false, none.clone()));
            let df = mp("df");
            assert_eq!(df.member, bf.member);
            assert_eq!((df.derived, shape(&df).2), (false, vec!["D".to_owned()]));
            // `D`'s own member, and a conversion there and back.
            assert_eq!(shape(&mp("dz")).2, none);
            assert_eq!(mp("back"), bx);
            // The translation unit's own constant.
            let cap = RunBudget::PROGRAM.result_bytes;
            let v = s.variable_value_named("dx_v", cap).expect("dx_v");
            assert_eq!(v.root, RunValue::MemberPointer(Box::new(dx)));
        },
    );
}

#[test]
fn reflections() {
    session(
        "constexpr int g = 3;\n\
         namespace N {}\n\
         template <class T> struct W { T v; };\n\
         consteval info t() { return ^^int; }\n\
         consteval info d() { return ^^g; }\n\
         consteval info n() { return ^^N; }\n\
         consteval info null() { return info{}; }\n\
         consteval info value() { return std::meta::reflect_constant(42); }\n\
         consteval info made() { info a[1] = {^^short}; return std::meta::substitute(^^W, a); }\n\
         consteval info old() { info a[1] = {^^int}; return std::meta::substitute(^^W, a); }\n\
         W<int> wi{};\n",
        |s| {
            let info = |v: RunValue| match v {
                RunValue::Info(i) => *i,
                other => panic!("not an info: {other:?}"),
            };
            let i = info(val(s, "t"));
            assert_eq!(
                (i.kind, &*i.display, i.decl, &i.value),
                (InfoKind::Type, "int", None, &None)
            );
            let i = info(val(s, "d"));
            assert_eq!((i.kind, &*i.display), (InfoKind::Declaration, "g"));
            assert!(i.decl.is_some());
            let i = info(val(s, "n"));
            assert_eq!((i.kind, &*i.display), (InfoKind::Namespace, "N"));
            assert!(i.decl.is_some());
            assert_eq!(info(val(s, "null")).kind, InfoKind::Null);
            let i = info(val(s, "value"));
            assert_eq!((i.kind, i.value), (InfoKind::Value, Some(RunValue::Signed(42))));
            // `W<short>` was made by the run: no `decl` (spec §4.1), on
            // every run; `W<int>` existed at open.
            for _ in 0..2 {
                let i = info(val(s, "made"));
                assert_eq!((i.kind, &*i.display, i.decl), (InfoKind::Type, "W<short int>", None));
            }
            let i = info(val(s, "old"));
            assert_eq!(i.kind, InfoKind::Type);
            assert!(i.decl.is_some(), "{i:?}");
        },
    );
}

#[test]
fn the_constants_table_and_object_identity() {
    session(
        "struct Span { const char* b; const char* e; };\n\
         consteval Span span() { const char* s = \"hello\"; return {s, s + 5}; }\n\
         consteval const char* lit() { return \"ab\"; }\n\
         struct Two { const char* x; const char* y; };\n\
         consteval Two apart() { return {\"ab\", \"ab\"}; }\n\
         consteval Two once() { return {lit(), lit()}; }\n\
         consteval Two statics() {\n\
           return {std::meta::extract<const char*>(std::meta::__kiln_reflect_constant_string(\"q\")),\n\
                   std::meta::extract<const char*>(std::meta::__kiln_reflect_constant_string(\"q\"))};\n\
         }\n\
         struct P { int a, b; };\n\
         template <P v> consteval const P* tpo() { return &v; }\n\
         struct Many { const P* p[64]; const P* other; };\n\
         consteval Many many() { Many m{}; for (auto& q : m.p) q = tpo<P{1, 2}>(); \
           m.other = tpo<P{3, 4}>(); return m; }\n\
         struct Q { const P* x; const P* y; const P* again; };\n\
         template <Q q> consteval const Q* tpo2() { return &q; }\n\
         consteval const Q* nested() {\n\
           return tpo2<Q{tpo<P{7, 8}>(), tpo<P{5, 6}>(), tpo<P{7, 8}>()}>();\n\
         }\n",
        |s| {
            let text = |t: &[u8]| RunValue::Text { unit: CharUnit::Char, data: t.into() };
            let cidx = |v: &RunValue| match pointer(v).base {
                PointerBase::Constant(i) => i,
                ref other => panic!("not a constant: {other:?}"),
            };
            // A span-like pair into one string literal: one object, one
            // index.
            let t = tree(s, "span");
            let (b, e) = (t.root.field("b").unwrap(), t.root.field("e").unwrap());
            assert_eq!((cidx(b), cidx(e)), (0, 0));
            assert_eq!(&*pointer(b).path, &[PathStep::Index(0)][..]);
            assert_eq!(&*pointer(e).path, &[PathStep::Index(5)][..]);
            assert_eq!(t.constants, vec![text(b"hello\0")]);
            // Two equal literals written apart are two objects
            // ([lex.string]/9): two indices, equal values.
            let t = tree(s, "apart");
            assert_eq!(
                (cidx(t.root.field("x").unwrap()), cidx(t.root.field("y").unwrap())),
                (0, 1)
            );
            assert_eq!(t.constants, vec![text(b"ab\0"), text(b"ab\0")]);
            // Two evaluations of one literal expression: one object.
            let t = tree(s, "once");
            assert_eq!(
                (cidx(t.root.field("x").unwrap()), cidx(t.root.field("y").unwrap())),
                (0, 0)
            );
            assert_eq!(t.constants.len(), 1);
            // `define_static_string` is interned by content: one object.
            let t = tree(s, "statics");
            assert_eq!(
                (cidx(t.root.field("x").unwrap()), cidx(t.root.field("y").unwrap())),
                (0, 0)
            );
            assert_eq!(t.constants.len(), 1);
            // One template parameter object per value: 64 pointers, one
            // constant decoded once, then the other value's.
            let t = tree(s, "many");
            let RunValue::Array(ps) = t.root.field("p").unwrap() else { panic!("p") };
            assert!(ps.iter().all(|p| cidx(p) == 0 && pointer(p).path.is_empty()));
            assert_eq!(cidx(t.root.field("other").unwrap()), 1);
            assert_eq!(t.constants.len(), 2);
            assert_eq!(t.constants[0].field("b"), Some(&RunValue::Signed(2)));
            assert_eq!(t.constants[1].field("a"), Some(&RunValue::Signed(3)));
            // Pointers reached inside a constant give the next indices, in
            // order.
            let t = tree(s, "nested");
            assert_eq!(cidx(&t.root), 0);
            let q = &t.constants[0];
            let idx = |n: &str| cidx(q.field(n).unwrap());
            assert_eq!((idx("x"), idx("y"), idx("again")), (1, 2, 1));
            assert_eq!(t.constants.len(), 3);
            assert_eq!(t.constants[1].field("a"), Some(&RunValue::Signed(7)));
            assert_eq!(t.constants[2].field("a"), Some(&RunValue::Signed(5)));
        },
    );
}

#[test]
fn refused_types() {
    session(
        "struct VB { int a; };\n\
         struct HasV : virtual VB {};\n\
         struct Inner { HasV h; };\n\
         struct NonLit { NonLit(); int a; };\n\
         typedef int v4 __attribute__((vector_size(16)));\n\
         inline constexpr int k = 1;\n\
         consteval const int& ref() { return k; }\n\
         consteval HasV hv();\n\
         consteval Inner inner();\n\
         constexpr NonLit nonlit();\n\
         consteval _BitInt(129) wide();\n\
         consteval v4 vec();\n\
         struct RefMember { const int& r; };\n\
         consteval RefMember ok() { return {k}; }\n",
        |s| {
            for name in ["ref", "nonlit", "wide", "vec"] {
                assert_eq!(s.entry_named(name).unwrap_err(), EntryError::WrongReturnType, "{name}");
            }
            // WP-2210: a class with a virtual base is a result type (these
            // two are only undefined).
            for name in ["hv", "inner"] {
                assert_eq!(s.entry_named(name).unwrap_err(), EntryError::Undefined, "{name}");
            }
            assert!(s.entry_named("ok").is_ok());
        },
    );
}

#[test]
fn refused_values_fail_the_run() {
    session(
        "struct H { int* p; };\n\
         consteval H heap() { return H{new int(1)}; }\n\
         constexpr H leak() { return H{new int(2)}; }\n\
         struct Uninit { int a; int b; constexpr Uninit() : a(1) {} };\n\
         consteval Uninit uninit() { return {}; }\n",
        |s| {
            for name in ["heap", "leak", "uninit"] {
                let r = run_budget(s, name, RunBudget::PROGRAM);
                assert_eq!(r.result, Err(RunError::Failed), "{name}");
                assert!(r.constants.is_empty());
                assert!(!r.diagnostics.is_empty(), "{name}: the freezer's notes");
            }
            // The session is usable after each.
            let r = run_budget(s, "heap", RunBudget::PROGRAM);
            assert_eq!(r.result, Err(RunError::Failed));
        },
    );
}

#[test]
fn the_size_cap() {
    session(
        "struct Big { int v[100000]; };\n\
         consteval Big big() { Big b{}; b.v[99999] = 9; return b; }\n\
         struct Chars { char c[1000000]; };\n\
         consteval Chars chars() { return {}; }\n",
        |s| {
            let elems = 100_000 * 32;
            let r = run_budget(s, "big", RunBudget::PROGRAM.with_result_bytes(elems));
            assert_eq!(r.result, Err(RunError::ResultTooLarge));
            let text: Vec<String> = r
                .diagnostics
                .iter()
                .flat_map(|d| d.notes.iter().map(|n| n.message.clone()))
                .collect();
            assert!(
                text.iter().any(
                    |m| m.contains("the result needs more than") && m.contains("result budget")
                ),
                "{text:?}"
            );
            let r = run_budget(s, "big", RunBudget::PROGRAM.with_result_bytes(elems + 4096));
            let Ok(RunValue::Record(b)) = &r.result else { panic!("{:?}", r.result) };
            let RunValue::Array(v) = &b.fields[0].value else { panic!("v") };
            assert_eq!((v.len(), &v[99_999]), (100_000, &RunValue::Signed(9)));
            // A character array is its bytes.
            let r = run_budget(s, "chars", RunBudget::PROGRAM.with_result_bytes(1_000_000 + 4096));
            assert!(r.result.is_ok(), "{:?}", r.diagnostics);
            // The session is usable after a refusal.
            let r = run_budget(s, "chars", RunBudget::PROGRAM.with_result_bytes(1000));
            assert_eq!(r.result, Err(RunError::ResultTooLarge));
            assert!(run_budget(s, "big", RunBudget::PROGRAM).result.is_ok());
        },
    );
}

const DETERMINISM: &str = "struct In { int x; const char* name; };\n\
     struct Out { In in[3]; double d; const In* first; info r; };\n\
     template <class T> struct Box { T v; };\n\
     constexpr const char* names[] = {\"a\", \"b\", \"c\"};\n\
     consteval Out out() {\n\
       Out o{};\n\
       for (int i = 0; i < 3; ++i) o.in[i] = {i, names[i]};\n\
       o.d = 0.25; o.first = nullptr; o.r = ^^Box<long>; return o;\n\
     }\n\
     consteval Box<int> boxed() { return {std::meta::extract<int>(std::meta::reflect_constant(5))}; }\n\
     consteval int other() { int n = 0; for (int i = 0; i < 100; ++i) n += i; return n; }\n";

#[test]
fn determinism_over_runs_and_sessions() {
    let fresh = session(DETERMINISM, |s| (tree(s, "out"), tree(s, "boxed")));
    session(DETERMINISM, |s| {
        // A used session: other entries first.
        assert_eq!(val(s, "other"), RunValue::Signed(4950));
        let boxed = tree(s, "boxed");
        let first = tree(s, "out");
        assert_eq!(first, fresh.0);
        assert_eq!(boxed, fresh.1);
        // Run 1 and run N.
        for _ in 0..20 {
            assert_eq!(tree(s, "out"), first);
        }
        // kept() stays flat.
        let _ = tree(s, "out");
        let k2 = s.kept();
        for _ in 0..1000 {
            let _ = run_budget(s, "out", RunBudget::PROGRAM);
        }
        assert_eq!(s.kept(), k2);
    });
}

#[test]
fn constexpr_entries_are_destroyed_and_consteval_ones_are_not() {
    session(
        "struct T { int v; constexpr ~T() { if (v == 13) (void)(1 / (v - 13)); } };\n\
         constexpr T bad() { return {13}; }\n\
         consteval T notdestroyed() { return {13}; }\n\
         constexpr T fine() { return {1}; }\n",
        |s| {
            assert_eq!(val(s, "fine").field("v"), Some(&RunValue::Signed(1)));
            assert_eq!(val(s, "notdestroyed").field("v"), Some(&RunValue::Signed(13)));
            let r = run_budget(s, "bad", RunBudget::PROGRAM);
            assert_eq!(r.result, Err(RunError::Failed));
        },
    );
}

#[test]
fn variable_values() {
    session(
        "struct In { int x; const char* s; };\n\
         consteval In make() { return {4, \"hi\"}; }\n\
         constexpr In v = make();\n\
         constexpr int i = 3;\n\
         const int ci = 4;\n\
         int notconst = 5;\n\
         const double cd = 1.0;\n\
         constexpr const int& r = i;\n\
         namespace n { constexpr int nested = 6; }\n\
         template <int N> struct L { static constexpr int lazy = N * 2; };\n\
         using SL = L<21>;\n\
         consteval int touch() { return SL::lazy; }\n\
         typedef int v4 __attribute__((vector_size(16)));\n\
         constexpr v4 vv{};\n\
         void f();\n",
        |s| {
            let cap = RunBudget::PROGRAM.result_bytes;
            let before = s.variable_value_named("v", cap).expect("v");
            // `run(f)` and `variable_value(v)` give one tree.
            assert_eq!(tree(s, "make"), before);
            assert_eq!(tree(s, "make"), before);
            assert_eq!(s.variable_value_named("v", cap), Ok(before.clone()));
            assert_eq!(before.root.field("x"), Some(&RunValue::Signed(4)));
            assert_eq!(s.variable_value_named("i", cap).map(|t| t.root), Ok(RunValue::Signed(3)));
            assert_eq!(s.variable_value_named("ci", cap).map(|t| t.root), Ok(RunValue::Signed(4)));
            assert_eq!(
                s.variable_value_named("::n::nested", cap).map(|t| t.root),
                Ok(RunValue::Signed(6))
            );
            assert_eq!(s.variable_value_named("notconst", cap), Err(ValueError::NotAConstant));
            assert_eq!(s.variable_value_named("cd", cap), Err(ValueError::NotAConstant));
            assert_eq!(s.variable_value_named("f", cap), Err(ValueError::NotAVariable));
            assert_eq!(s.variable_value_named("vv", cap), Err(ValueError::WrongType));
            assert!(matches!(
                s.variable_value_named("missing", cap),
                Err(ValueError::NotFound { .. })
            ));
            assert!(matches!(s.variable_value_named("v", 8), Err(ValueError::TooLarge { cap: 8 })));
            let rt = s.variable_value_named("r", cap).expect("r");
            assert!(
                matches!(&pointer(&rt.root).base, PointerBase::Variable(e) if &*e.display == "i")
            );
            // On demand: an initializer never used is evaluated now, and
            // a later run that reaches it gives the same value.
            let lazy = s.variable_value_named("SL::lazy", cap);
            assert_eq!(lazy.map(|t| t.root), Ok(RunValue::Signed(42)));
            assert_eq!(val(s, "touch"), RunValue::Signed(42));
            assert_eq!(
                s.variable_value_named("SL::lazy", cap).map(|t| t.root),
                Ok(RunValue::Signed(42))
            );
        },
    );
}

/// WP-2420 (spec/eval-session.md §4.1 "On demand"): a static data member
/// whose initializer nothing instantiated (an in-class `constexpr` member
/// of a class template specialization, an out-of-line `const` one, one of
/// class type) has it instantiated and evaluated by `variable_value`, as
/// an odr-use in a constant expression would; a run reaching it later
/// gives the same value, and so does `variable_value` after the run. An
/// initializer that is not a constant expression is `NotAConstant`, its
/// diagnostics captured (none reaches the sink).
#[test]
fn variable_values_instantiate_on_demand() {
    // `sizeof(T)` keeps `UseE`'s uses dependent, so only a run
    // instantiates them (the target is x86-64: `sizeof(int)` is 4).
    session(
        "struct P { int a, b; };\n\
         template <int N> constexpr int tw() { return N * 2; }\n\
         template <int N> struct Lz { static constexpr int v = N * 3; static constexpr P p{N, N + 1};\n\
           static constexpr int w = tw<N>(); static const int k; };\n\
         template <int N> const int Lz<N>::k = tw<N>() + 1;\n\
         template <int N> struct Ol { static const int k; };\n\
         template <int N> const int Ol<N>::k = N + 1;\n\
         template <int N> struct Bad { static constexpr int v = 1 / (N - N); };\n\
         template <class T> struct UseE {\n\
           static consteval int f() { return Lz<sizeof(T) + 3>::v + Ol<sizeof(T) + 3>::k; }\n\
           static consteval int g() { return Lz<sizeof(T) + 5>::v + Ol<sizeof(T) + 5>::k; }\n\
         };\n\
         static_assert(sizeof(Lz<5>) + sizeof(Lz<7>) + sizeof(Lz<9>) > 0);\n\
         static_assert(sizeof(Ol<3>) + sizeof(Ol<7>) + sizeof(Ol<9>) > 0);\n\
         static_assert(sizeof(Bad<1>) + sizeof(UseE<int>) > 0);\n\
         using L5 = Lz<5>; using L7 = Lz<7>; using L9 = Lz<9>;\n\
         using O3 = Ol<3>; using O7 = Ol<7>; using O9 = Ol<9>;\n\
         using B1 = Bad<1>; using UE = UseE<int>;\n",
        |s| {
            let cap = RunBudget::PROGRAM.result_bytes;
            let int = |s: &mut EvalSession<'_, '_>, n: &str| {
                s.variable_value_named(n, cap).map(|t| t.root)
            };
            assert_eq!(int(s, "L5::v"), Ok(RunValue::Signed(15)));
            assert_eq!(int(s, "O3::k"), Ok(RunValue::Signed(4)));
            let p = s.variable_value_named("L5::p", cap).expect("L5::p");
            assert_eq!(p.root.field("a"), Some(&RunValue::Signed(5)));
            assert_eq!(p.root.field("b"), Some(&RunValue::Signed(6)));
            // An initializer calling a function template nothing
            // instantiated: the instantiation's evaluation instantiates it.
            assert_eq!(int(s, "L5::w"), Ok(RunValue::Signed(10)));
            assert_eq!(int(s, "L5::k"), Ok(RunValue::Signed(11)));
            assert_eq!(int(s, "B1::v"), Err(ValueError::NotAConstant));
            // Read first, then reached by a run, then read again.
            assert_eq!(int(s, "L7::v"), Ok(RunValue::Signed(21)));
            assert_eq!(int(s, "O7::k"), Ok(RunValue::Signed(8)));
            assert_eq!(val(s, "UE::f"), RunValue::Signed(29));
            assert_eq!(int(s, "L7::v"), Ok(RunValue::Signed(21)));
            assert_eq!(int(s, "O7::k"), Ok(RunValue::Signed(8)));
            // Reached by a run first, then read.
            assert_eq!(val(s, "UE::g"), RunValue::Signed(37));
            assert_eq!(int(s, "L9::v"), Ok(RunValue::Signed(27)));
            assert_eq!(int(s, "O9::k"), Ok(RunValue::Signed(10)));
            assert_eq!(val(s, "UE::g"), RunValue::Signed(37));
            // The answers do not change.
            assert_eq!(int(s, "L5::v"), Ok(RunValue::Signed(15)));
            assert_eq!(int(s, "O3::k"), Ok(RunValue::Signed(4)));
            assert_eq!(int(s, "B1::v"), Err(ValueError::NotAConstant));
        },
    );
}

/// WP-2420: a read whose on-demand instantiation fails leaves the
/// failure as a run's would: a later run that reaches the initializer
/// fails with the same diagnostics as in a fresh session.
#[test]
fn a_failed_on_demand_read_fails_later_runs_alike() {
    const SRC: &str = "template <int N> struct Bad { static constexpr int v = 1 / (N - N); };\n\
         template <class T> struct UseB { static consteval int f() { return Bad<sizeof(T)>::v; } };\n\
         static_assert(sizeof(Bad<4>) + sizeof(UseB<int>) > 0);\n\
         using B2 = Bad<4>; using UB = UseB<int>;\n";
    let msgs = |r: &RunReport| -> Vec<String> {
        fn one(d: &Diagnostic, out: &mut Vec<String>) {
            out.push(format!("{:?} {:?}: {}", d.level, d.span, d.message));
            for n in &d.notes {
                one(n, out);
            }
        }
        let mut out = Vec::new();
        for d in &r.diagnostics {
            one(d, &mut out);
        }
        out
    };
    let fresh = session(SRC, |s| {
        let r = run_budget(s, "UB::f", RunBudget::PROGRAM);
        assert_eq!(r.result, Err(RunError::Failed));
        msgs(&r)
    });
    assert!(fresh.iter().any(|m| m.contains("division by zero")), "{fresh:#?}");
    session(SRC, |s| {
        let cap = RunBudget::PROGRAM.result_bytes;
        assert_eq!(s.variable_value_named("B2::v", cap), Err(ValueError::NotAConstant));
        for _ in 0..2 {
            let r = run_budget(s, "UB::f", RunBudget::PROGRAM);
            assert_eq!(r.result, Err(RunError::Failed));
            assert_eq!(msgs(&r), fresh);
        }
        assert_eq!(s.variable_value_named("B2::v", cap), Err(ValueError::NotAConstant));
    });
}

/// The parts of `RunError::Uncaught` of a run of `name`, which must end so.
#[track_caller]
fn uncaught(
    s: &mut EvalSession<'_, '_>,
    name: &str,
) -> (kiln_parse::Entity, Vec<u64>, Result<ValueTree, String>, Vec<String>) {
    // The tree unboxed, to compare with a result's.
    let r = run_budget(s, name, RunBudget::PROGRAM);
    let msgs: Vec<String> = r.diagnostics.iter().map(|d| d.message.clone()).collect();
    match r.result {
        Err(RunError::Uncaught { ty, scalars, value, .. }) => {
            let mut notes = Vec::new();
            for d in &r.diagnostics {
                notes.extend(d.notes.iter().map(|n| n.message.clone()));
            }
            (ty, scalars, value.map(|v| *v), notes)
        }
        other => panic!("{name}: {other:?} {msgs:?}"),
    }
}

/// WP-2480 and WP-2502 (spec/eval-session.md §8, [except.throw]/3): an
/// exception of a namespaced class type leaving the entry names its type
/// fully qualified, with the class's declaration, and hands over the
/// thrown object's value as a run's result of that type would be read
/// (the value the note renders); a class of the same name in another
/// namespace is told apart by both.
#[test]
fn an_uncaught_class_has_its_qualified_type_declaration_and_value() {
    session(
        "namespace ns::v0 { struct error { int code; int line; }; }\n\
         namespace other { struct error { int code; int line; }; }\n\
         consteval ns::v0::error make() { return {2, 1}; }\n\
         consteval other::error make_other() { return {2, 1}; }\n\
         constexpr void fail(int c) { throw ns::v0::error{c, 1}; }\n\
         consteval int throws() { fail(2); return 0; }\n\
         consteval int throws_other() { throw other::error{2, 1}; }\n",
        |s| {
            let made = tree(s, "make");
            let made_other = tree(s, "make_other");
            let decl = record(&made.root).ty.decl;
            assert!(decl.is_some());
            for _ in 0..2 {
                let (ty, scalars, value, notes) = uncaught(s, "throws");
                assert_eq!((&*ty.display, ty.decl), ("ns::v0::error", decl));
                assert_eq!(scalars, vec![2, 1]);
                assert_eq!(value.as_ref(), Ok(&made), "{value:?}");
                assert!(
                    notes.iter().any(|n| n.contains("of type 'ns::v0::error' with content {2, 1}")),
                    "{notes:?}"
                );
                let (ty, _, value, _) = uncaught(s, "throws_other");
                assert_eq!(&*ty.display, "other::error");
                assert_eq!(ty.decl, record(&made_other.root).ty.decl);
                assert_ne!(ty.decl, decl);
                assert_eq!(value, Ok(made_other.clone()));
            }
        },
    );
}

/// WP-2502 ([expr.throw]/4, [except.handle]/16): `throw;` rethrows the
/// handled exception object itself, so the value handed over is that
/// object's as the handler left it; `throw e;` in a handler throws a new
/// object, whose value is handed over.
#[test]
fn a_rethrown_exception_hands_over_the_rethrown_value() {
    session(
        "struct boxed { int v; };\n\
         constexpr void inner() { throw boxed{1}; }\n\
         consteval int rethrow() { try { inner(); } catch (boxed& b) { b.v = 7; throw; } return 0; }\n\
         consteval int rethrow_new() { try { inner(); } catch (boxed b) { throw boxed{b.v + 40}; } return 0; }\n\
         consteval int nested() { try { try { inner(); } catch (boxed& b) { b.v = 2; throw; } } \
           catch (boxed& b) { b.v *= 10; throw; } return 0; }\n\
         consteval boxed seven() { return {7}; }\n\
         consteval boxed forty_one() { return {41}; }\n\
         consteval boxed twenty() { return {20}; }\n",
        |s| {
            let want = [("rethrow", "seven"), ("rethrow_new", "forty_one"), ("nested", "twenty")];
            for (entry, made) in want {
                let made = tree(s, made);
                let (ty, scalars, value, _) = uncaught(s, entry);
                assert_eq!(&*ty.display, "boxed", "{entry}");
                assert_eq!(ty.decl, record(&made.root).ty.decl, "{entry}");
                assert_eq!(value, Ok(made.clone()), "{entry}");
                assert_eq!(
                    made.root.field("v").and_then(RunValue::as_i128),
                    Some(i128::from(scalars[0]))
                );
            }
        },
    );
}

/// WP-2480 and WP-2502 ([except.throw]/3, [except.handle]/16): the type
/// handed over is the exception object's: a class derived from another
/// exception class keeps its own type and its base subobject's value; a
/// `throw` of a base reference to a derived object copies the base
/// (the operand's static type); a rethrow of a derived object caught by a
/// base reference rethrows the derived object.
#[test]
fn a_derived_exception_type_is_its_own() {
    session(
        "namespace app { struct base_error { int code; }; \
         struct derived_error : base_error { int extra; }; }\n\
         using namespace app;\n\
         consteval derived_error make_derived() { return {{3}, 4}; }\n\
         consteval base_error make_base() { return {3}; }\n\
         consteval int derived() { throw derived_error{{3}, 4}; }\n\
         consteval int sliced() { derived_error d{{3}, 4}; base_error& r = d; throw r; }\n\
         consteval int rethrown() { try { throw derived_error{{3}, 4}; } catch (base_error&) { throw; } }\n",
        |s| {
            let derived = tree(s, "make_derived");
            let base = tree(s, "make_base");
            assert_eq!(
                names(&derived.root),
                vec![
                    (FieldKind::Base, "app::base_error".into()),
                    (FieldKind::Member, "extra".into())
                ]
            );
            for (entry, want, display) in [
                ("derived", &derived, "app::derived_error"),
                ("sliced", &base, "app::base_error"),
                ("rethrown", &derived, "app::derived_error"),
            ] {
                let (ty, _, value, _) = uncaught(s, entry);
                assert_eq!(&*ty.display, display, "{entry}");
                assert_eq!(ty.decl, record(&want.root).ty.decl, "{entry}");
                assert_eq!(value.as_ref(), Ok(want), "{entry}");
            }
        },
    );
}

/// WP-2502 (spec/eval-session.md §8): a thrown object that has no value a
/// result could have (a pointer to an object the run allocated, a member
/// not initialized) hands over no value, with the reason; a pointer to a
/// string literal is a pointer into the constants table, as in a result.
#[test]
fn an_unrepresentable_thrown_value_says_why() {
    session(
        "struct holder { int* p; };\n\
         struct gap { int a; int b; constexpr gap() : a(1) {} };\n\
         struct message { const char* text; int n; };\n\
         consteval int heap() { throw holder{new int(1)}; }\n\
         consteval int partial() { throw gap(); }\n\
         consteval int literal() { throw message{\"boom\", 2}; }\n",
        |s| {
            for _ in 0..2 {
                let (ty, scalars, value, _) = uncaught(s, "heap");
                assert_eq!((&*ty.display, scalars.len()), ("holder", 0));
                let why = value.expect_err("a pointer to the run's heap");
                assert_eq!(why, "pointer to heap-allocated object is not a constant expression");
                let (_, scalars, value, _) = uncaught(s, "partial");
                assert_eq!(scalars, vec![1]);
                let why = value.expect_err("a member not initialized");
                assert_eq!(why, "subobject 'b' is not initialized");
                let (_, _, value, _) = uncaught(s, "literal");
                let v = value.expect("a pointer to a string literal");
                let p = pointer(&record(&v.root).fields[0].value);
                assert!(matches!(p.base, PointerBase::Constant(0)), "{p:?}");
                assert!(matches!(v.constants[0], RunValue::Text { .. }), "{:?}", v.constants);
            }
        },
    );
}
