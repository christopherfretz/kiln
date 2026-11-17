//! The definition hash of the link-time ODR checks (`kiln_abi::odr_def`;
//! spec/abi.md "Definition hash (`odr_def`)"; mortar `spec/odr.md`
//! O-10..O-13), checked five ways:
//!
//! - must-equal pairs: sources that differ only in what O-10 does not
//!   hash (spelling, macros, typedefs, parentheses, implicit `this`,
//!   implicit specifiers, how a unit uses the function);
//! - the mutation table: each row a pair of definitions that must differ;
//! - the golden file `mason_def_hash.golden`, which pins the bytes;
//! - over a library-heavy unit with libstdc++ and libc++: no name fed to
//!   the hash has a per-TU component, and two units that instantiate
//!   different things agree on every definition they share;
//! - deep trees do not overflow the stack.
//!
//! `cost_report` (ignored; run in release) prints what hashing every
//! function definition of a unit costs against parsing it.

#![allow(clippy::cast_precision_loss, reason = "percentages and rates in printed reports")]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kiln_abi::mangle::{self, AnonIds};
use kiln_abi::odr_def::{DefDigest, DefHasher, ODR_DEF_SCHEME, is_templated_def};
use kiln_abi::{Arch, Target};
use kiln_ast::{Ast, DeclId};
use kiln_diag::{DiagEngine, DiagOptions, Level, MemorySink};
use kiln_lex::Std;
use kiln_parse::{ConstexprLimits, ParseOptions, ParsedTu, parse_tu};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

fn host_arch() -> Arch {
    if cfg!(target_arch = "aarch64") { Arch::AArch64 } else { Arch::X86_64 }
}

fn parse_opts(name: &str, src: String, opts: &ParseOptions) -> (SourceManager, ParsedTu) {
    let sm = SourceManager::new();
    let file = sm.add_virtual(name, src).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let tu = parse_tu(
        &sm,
        &interner,
        file,
        &engine,
        Target::for_arch(host_arch()),
        opts,
        ConstexprLimits::default(),
    );
    let errors: Vec<String> =
        sink.take().into_iter().filter(|d| d.level >= Level::Error).map(|d| d.message).collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    drop(engine);
    (sm, tu)
}

fn parse(src: &str) -> (SourceManager, ParsedTu) {
    parse_opts("t.cpp", src.to_owned(), &ParseOptions::default())
}

fn ident(ast: &Ast, d: DeclId) -> &str {
    ast.decls.get(d).name().as_ident().map_or("", |s| ast.symbols().as_str(s))
}

/// The function definitions of `ast` that are hashed: every one with a
/// body that is not a template's own.
fn definitions(ast: &Ast) -> Vec<DeclId> {
    ast.decls
        .ids()
        .filter(|&d| ast.decls.function(d).is_some_and(|f| f.body.is_some()))
        .filter(|&d| !is_templated_def(ast, d))
        .collect()
}

/// The digest of the one definition named `name` in `src`.
fn digest_in(src: &str, name: &str) -> DefDigest {
    let (sm, tu) = parse(src);
    digest_of(&sm, &tu.ast, name)
}

fn digest_of(sm: &SourceManager, ast: &Ast, name: &str) -> DefDigest {
    let defs: Vec<DeclId> =
        definitions(ast).into_iter().filter(|&d| ident(ast, d) == name).collect();
    assert_eq!(defs.len(), 1, "one definition of `{name}`");
    DefHasher::new(ast, sm, AnonIds::new()).function(defs[0])
}

// ---------------------------------------------------------------- (a)

/// Pairs that must give the same digest of `f`, and the same `partial`.
const MUST_EQUAL: &[(&str, &str, &str)] = &[
    // N16: comments, whitespace, ignored attributes, macro names.
    (
        "N16 comments and whitespace",
        "int f(int a) { return a + 1; }",
        "int f(int a) /* c */ {\n  // c\n  return a\n   + 1;\n}",
    ),
    (
        "N16 ignored attributes",
        "int f(int a) { if (a) return 1; int b = a; return b; }",
        "[[nodiscard]] [[deprecated]] int f([[maybe_unused]] int a) { if (a) [[likely]] return 1; int b = a; return b; }",
    ),
    (
        "N16 macros that expand alike",
        "#define ONE 1\nint f(int a) { return a + ONE; }",
        "#define UNO 1\nint f(int a) { return a + UNO; }",
    ),
    ("literal: decimal and hex", "int f() { return 16; }", "int f() { return 0x10; }"),
    ("literal: decimal and binary", "int f() { return 16; }", "int f() { return 0b10000; }"),
    ("literal: digit separator", "int f() { return 16; }", "int f() { return 1'6; }"),
    ("literal: octal", "int f() { return 8; }", "int f() { return 010; }"),
    ("literal: float spelling", "double f() { return 1.5; }", "double f() { return 15e-1; }"),
    ("literal: char spelling", "int f() { return 'a'; }", "int f() { return '\\x61'; }"),
    (
        "literal: string spelling",
        "const char *f() { return \"ab\"; }",
        "const char *f() { return \"a\\x62\"; }",
    ),
    (
        "macro against its expansion",
        "#define TWICE(x) ((x) + (x))\nint f(int a) { return TWICE(a); }",
        "int f(int a) { return ((a) + (a)); }",
    ),
    (
        "object-like and function-like macro",
        "#define E a * 2\nint f(int a) { return E; }",
        "#define E() a * 2\nint f(int a) { return E(); }",
    ),
    (
        "__LINE__ at two lines",
        "int f() { return __LINE__; }",
        "\n\n\n\nint f() { return __LINE__; }",
    ),
    (
        "__LINE__ through a macro",
        "int f() { return __LINE__; }",
        "#define L __LINE__\n\nint f() { return L; }",
    ),
    (
        "__FILE__ in two files",
        "const char *f() { return __FILE__; }",
        "#line 7 \"other.cpp\"\nconst char *f() { return __FILE__; }",
    ),
    (
        "__FILE__ through a macro",
        "const char *f() { return __FILE__; }",
        "#define F __FILE__\nconst char *f() { return F; }",
    ),
    ("__func__", "const char *f() { return __func__; }", "\nconst char *f() { return __func__; }"),
    (
        "stringized __LINE__ concatenation",
        "#define S2(x) #x\n#define S(x) S2(x)\nconst char *f() { return \"at \" S(__LINE__); }",
        "#define S2(x) #x\n#define S(x) S2(x)\n\nconst char *f() { return \"at \" S(__LINE__); }",
    ),
    (
        "typedef against the type it names",
        "typedef long L; L f(L a) { L b = a; return b; }",
        "long f(long a) { long b = a; return b; }",
    ),
    (
        "alias against the type it names",
        "using P = int *; P f(P a) { return a; }",
        "int *f(int *a) { return a; }",
    ),
    (
        "parameter names",
        "int f(int a, int b) { return a - b; }",
        "int f(int x, int y) { return x - y; }",
    ),
    ("local names", "int f() { int a = 1; return a; }", "int f() { int z = 1; return z; }"),
    (
        "redundant parentheses",
        "int f(int a, int b) { return a * b + 1; }",
        "int f(int a, int b) { return ((a * b)) + (1); }",
    ),
    (
        "implicit this",
        "struct S { int m; int f(); }; int S::f() { return m; }",
        "struct S { int m; int f(); }; int S::f() { return this->m; }",
    ),
    (
        "implicit this, member call",
        "struct S { int g(); int f(); }; int S::f() { return g(); }",
        "struct S { int g(); int f(); }; int S::f() { return this->g(); }",
    ),
    ("trailing return type", "int f(int a) { return a; }", "auto f(int a) -> int { return a; }"),
    ("deduced return type", "int f(int a) { return a; }", "auto f(int a) { return a; }"),
    (
        "inline written on a class member",
        "struct S { int f() { return 1; } }; int u = S().f();",
        "struct S { inline int f() { return 1; } }; int u = S().f();",
    ),
    (
        "noexcept(true) and noexcept",
        "int f() noexcept { return 1; }",
        "int f() noexcept(true) { return 1; }",
    ),
    (
        "noexcept(false) and nothing",
        "int f() { return 1; }",
        "int f() noexcept(false) { return 1; }",
    ),
    (
        "contract assertion (X16)",
        "int f(int a) { return a; }",
        "int f(int a) { contract_assert(a > 0); return a; }",
    ),
    (
        "block-scope typedef and static_assert",
        "int f(int a) { return a; }",
        "int f(int a) { typedef int I; static_assert(sizeof(I) == sizeof(int)); return a; }",
    ),
    (
        "enumerator through a using-enum",
        "enum class E { A, B }; int f() { return int(E::B); }",
        "enum class E { A, B }; int f() { using enum E; return int(B); }",
    ),
    (
        "sizeof a type and a typedef of it",
        "unsigned long f() { return sizeof(int); }",
        "typedef int I; unsigned long f() { return sizeof(I); }",
    ),
    (
        "qualified call of a non-virtual member",
        "struct S { int g(); int f(); }; int S::f() { return g(); }",
        "struct S { int g(); int f(); }; int S::f() { return S::g(); }",
    ),
    // K-3d: closures by their resolved captures; specifiers as written.
    (
        "lambda: [=] and [x] capturing only x",
        "int f(int x) { return [=] { return x; }(); }",
        "int f(int x) { return [x] { return x; }(); }",
    ),
    (
        "lambda: [&] and [&x] capturing only x",
        "int f(int x) { return [&] { return x; }(); }",
        "int f(int x) { return [&x] { return x; }(); }",
    ),
    (
        "lambda: constexpr written on the call operator is not hashed (N25)",
        "int f() { return [] { return 1; }(); }",
        "int f() { return []() constexpr { return 1; }(); }",
    ),
    (
        "lambda: parameter and capture-default spellings",
        "int f(int x) { return [=](int a) { return a + x; }(1); }",
        "int f(int x) { return [x](int b) { return b + x; }(1); }",
    ),
    (
        "lambda: closure named by index, local typedef transparent",
        "int f() { auto l = [] { return 1; }; decltype(l) &r = l; return r(); }",
        "int f() { auto m = [] { return 1; }; typedef decltype(m) M; M &q = m; return q(); }",
    ),
    (
        "local class: names of the class and its parameters",
        "int f() { struct L { int g(int a) { return a; } }; return L().g(1); }",
        "int f() { struct K { int g(int b) { return b; } }; return K().g(1); }",
    ),
    (
        "contract assertion skipped (X16)",
        "int f(int a) { return [a] { return a; }(); }",
        "int f(int a) { contract_assert(a > 0); return [a] { contract_assert(a); return a; }(); }",
    ),
];

#[test]
fn must_equal_pairs() {
    let mut bad = Vec::new();
    for &(case, a, b) in MUST_EQUAL {
        let (x, y) = (digest_in(a, "f"), digest_in(b, "f"));
        if x != y {
            bad.push(format!("{case}: {x:x?} != {y:x?}"));
        }
    }
    assert!(bad.is_empty(), "must-equal pairs differ:\n{}", bad.join("\n"));
}

/// N25: what kiln marks `constexpr` implicitly (an implicit destructor in
/// C++20, a closure's call operator from C++17) is not hashed: the
/// function using them hashes alike under every standard.
#[test]
fn n25_implicit_specifiers_across_standards() {
    let src = "struct M { ~M() {} };\nstruct X { M s; };\n\
               inline int f(int a) { X x; auto l = [a] { return a; }; return l(); }\n\
               int u = f(1);\n";
    let mut seen = Vec::new();
    for std in [Std::Cxx17, Std::Cxx20, Std::Cxx26] {
        let opts = ParseOptions { std, ..ParseOptions::default() };
        let (sm, tu) = parse_opts("t.cpp", src.to_owned(), &opts);
        let ast = &tu.ast;
        let mut h = DefHasher::new(ast, &sm, AnonIds::new());
        let f = definitions(ast).into_iter().find(|&d| ident(ast, d) == "f").unwrap();
        let x = definitions(ast).into_iter().filter(|&d| {
            ast.decls.get(d).kind() == kiln_ast::decl::DeclKind::CXXDestructor
                && ast.decls.get(d).ctx().is_some_and(|c| ident(ast, c) == "X")
        });
        let dtor: Vec<DefDigest> = x.map(|d| h.function(d)).collect();
        seen.push((h.function(f), dtor));
    }
    assert!(seen.windows(2).all(|w| w[0] == w[1]), "{seen:x?}");
}

/// O-10: lazily computed sema state is not hashed. The same inline
/// function, used in a constant expression, address-taken and with more
/// specializations instantiated in one unit, unreferenced in the other.
#[test]
fn use_in_the_unit_does_not_matter() {
    let common = "template <class T> int t(T x) { return int(x) + 1; }\n\
                  constexpr int f(int a) { return a * 2; }\n\
                  inline int g(int a) { return t(a) + f(a); }\n";
    let used = format!(
        "{common}static_assert(f(2) == 4);\nint (*p)(int) = &f;\nint (*q)(int) = &g;\n\
         int u = t(1L) + t('c') + t(2.0) + g(3);\nconstexpr int k = f(5);\n"
    );
    let (sm1, tu1) = parse(common);
    let (sm2, tu2) = parse(&used);
    for name in ["f", "g"] {
        assert_eq!(digest_of(&sm1, &tu1.ast, name), digest_of(&sm2, &tu2.ast, name), "{name}");
    }
    let t1 = by_mangled(&sm1, &tu1.ast);
    let t2 = by_mangled(&sm2, &tu2.ast);
    assert_eq!(t1.get("_Z1tIiEiT_"), t2.get("_Z1tIiEiT_"));
    assert!(t2.contains_key("_Z1tIlEiT_"));
}

fn by_mangled(sm: &SourceManager, ast: &Ast) -> BTreeMap<String, DefDigest> {
    let mut h = DefHasher::new(ast, sm, AnonIds::new());
    let mut anon = AnonIds::new();
    definitions(ast)
        .into_iter()
        .map(|d| (mangle::mangle_decl(ast, &mut anon, d), h.function(d)))
        .collect()
}

// ---------------------------------------------------------------- (b)

/// Declarations every mutation row sees.
const PRELUDE: &str = "struct S { int m; int n; int g(int); virtual int v(); static int st; };\n\
                       enum E { EA, EB };\nint g(int); int h(int); int gl; long gl2;\n\
                       struct It { int operator*(); It &operator++(); bool operator!=(It); };\n\
                       struct R { It begin(); It end(); };\n";

/// Each row: a case, then two definitions of `f` that must differ. One
/// row a line; later packages add rows for the kinds they cover.
const MUST_DIFFER: &str = r#"
literal value | int f() { return 1; } | int f() { return 2; }
literal type | long f() { return 16; } | long f() { return 16u; }
literal type long | long f() { return 16; } | long f() { return 16l; }
float value | double f() { return 1.0; } | double f() { return 1.5; }
float type | double f() { return 1.0; } | double f() { return 1.0f; }
char value | int f() { return 'a'; } | int f() { return 'b'; }
char type | int f() { return 'a'; } | int f() { return u'a'; }
string value | const char *f() { return "ab"; } | const char *f() { return "ac"; }
string length | const char *f() { return "ab"; } | const char *f() { return "ab\0"; }
bool value | bool f() { return true; } | bool f() { return false; }
nullptr vs 0 | int *f() { return nullptr; } | int *f() { return 0; }
callee | int f(int a) { return g(a); } | int f(int a) { return h(a); }
argument | int f(int a, int b) { return g(a); } | int f(int a, int b) { return g(b); }
argument order | int f(int a, int b) { return a - b; } | int f(int a, int b) { return b - a; }
global | int f() { return gl; } | int f() { return (int)gl2; }
member | int f(S s) { return s.m; } | int f(S s) { return s.n; }
arrow | int f(S *p) { return p->m; } | int f(S *p) { return (*p).m; }
virtual call qualified | int f(S &s) { return s.v(); } | int f(S &s) { return s.S::v(); }
member call | int f(S &s) { return s.g(1); } | int f(S &s) { return s.g(2); }
static member | int f() { return S::st; } | int f() { return gl; }
enumerator | int f() { return EA; } | int f() { return EB; }
cast type | long f(int a) { return (long)a; } | long f(int a) { return (short)a; }
cast kind | long f(int a) { return static_cast<long>(a); } | long f(int a) { return (long)a; }
reinterpret vs static | char *f(void *p) { return static_cast<char *>(p); } | char *f(void *p) { return (char *)(long *)p; }
const_cast | int *f(const int *p) { return const_cast<int *>(p); } | int *f(const int *p) { return (int *)p; }
local type | int f(int a) { int x = a; return x; } | int f(int a) { long x = a; return x; }
local const | int f(int a) { int x = a; return x; } | int f(int a) { const int x = a; return x; }
local static | int f(int a) { int x = 1; return x + a; } | int f(int a) { static int x = 1; return x + a; }
local init | int f() { int x = 1; return x; } | int f() { int x = 2; return x; }
local no init | int f() { int x; return 0; } | int f() { int x = 0; return 0; }
direct vs copy init | int f() { int x = 1; return x; } | int f() { int x(1); return x; }
statement order | void f(int &a, int &b) { a = 1; b = 2; } | void f(int &a, int &b) { b = 2; a = 1; }
dropped statement | void f(int &a, int &b) { a = 1; b = 2; } | void f(int &a, int &b) { a = 1; }
if vs if not | int f(int a) { if (a) return 1; return 0; } | int f(int a) { if (!a) return 1; return 0; }
if vs if-else | int f(int a) { if (a) return 1; return 0; } | int f(int a) { if (a) return 1; else return 0; }
if consteval | int f() { if consteval { return 1; } return 0; } | int f() { if !consteval { return 1; } return 0; }
if constexpr | int f() { if constexpr (true) { return 1; } return 0; } | int f() { if (true) { return 1; } return 0; }
while vs for | void f(int a) { while (a) --a; } | void f(int a) { for (; a;) --a; }
while vs do | void f(int a) { while (a) --a; } | void f(int a) { do --a; while (a); }
for parts | void f(int a) { for (int i = 0; i < a; ++i) g(i); } | void f(int a) { for (int i = 0; i < a; i++) g(i); }
range-for vs iterator loop | int f(R r) { int s = 0; for (int x : r) s += x; return s; } | int f(R r) { int s = 0; for (It i = r.begin(), e = r.end(); i != e; ++i) s += *i; return s; }
break vs continue | void f(int a) { while (a) { --a; break; } } | void f(int a) { while (a) { --a; continue; } }
switch case | int f(int a) { switch (a) { case 1: return 1; default: return 0; } } | int f(int a) { switch (a) { case 2: return 1; default: return 0; } }
case range | int f(int a) { switch (a) { case 1 ... 3: return 1; } return 0; } | int f(int a) { switch (a) { case 1 ... 4: return 1; } return 0; }
goto target | void f(int a) { x: if (a--) goto x; y: if (a--) goto x; } | void f(int a) { x: if (a--) goto x; y: if (a--) goto y; }
try catch type | int f() { try { return g(1); } catch (int) { return 1; } } | int f() { try { return g(1); } catch (long) { return 1; } }
catch all | int f() { try { return g(1); } catch (int) { return 1; } } | int f() { try { return g(1); } catch (...) { return 1; } }
conditional arms | int f(int a, int b, int c) { return a ? b : c; } | int f(int a, int b, int c) { return a ? c : b; }
comma | int f(int a, int b) { return (a, b); } | int f(int a, int b) { return (b, a); }
subscript | int f(int *p) { return p[0]; } | int f(int *p) { return p[1]; }
subscript order | int f(int *p, int i) { return p[i]; } | int f(int *p, int i) { return i[p]; }
sizeof type | unsigned long f() { return sizeof(int); } | unsigned long f() { return sizeof(long); }
sizeof expr vs type | unsigned long f(int a) { return sizeof a; } | unsigned long f(int a) { return sizeof(int); }
sizeof vs alignof | unsigned long f() { return sizeof(int); } | unsigned long f() { return alignof(int); }
address of | int *f(int &a) { return &a; } | int *f(int &a) { int *p = &a; return p; }
return type | int f() { return 1; } | long f() { return 1; }
parameter type | int f(int a) { return a; } | int f(long a) { return a; }
parameter count | int f(int a) { return a; } | int f(int a, int) { return a; }
noexcept | int f() { return 1; } | int f() noexcept { return 1; }
constexpr written | int f() { return 1; } | constexpr int f() { return 1; }
consteval written | constexpr int f() { return 1; } | consteval int f() { return 1; }
inline written | int f() { return 1; } | inline int f() { return 1; }
static written | int f() { return 1; } | static int f() { return 1; }
variadic | int f(int a, ...) { return a; } | int f(int a) { return a; }
const member | struct T { int f(); }; int T::f() { return 1; } | struct T { int f() const; }; int T::f() const { return 1; }
ref-qualifier | struct T { int f() &; }; int T::f() & { return 1; } | struct T { int f() &&; }; int T::f() && { return 1; }
virtual | struct T { int f(); }; int T::f() { return 1; } | struct T { virtual int f(); }; int T::f() { return 1; }
compound assign | void f(int &a, int b) { a += b; } | void f(int &a, int b) { a = a + b; }
increment | int f(int a) { return ++a; } | int f(int a) { return a++; }
logical vs bitwise | bool f(bool a, bool b) { return a && b; } | bool f(bool a, bool b) { return a & b; }
operator call | int f(It i) { return *i; } | int f(It i) { return *++i; }
lambda capture kind | int f(int a) { return [=] { return a; }(); } | int f(int a) { return [&] { return a; }(); }
lambda capture entity | int f(int a, int b) { return [a] { return 1; }(); } | int f(int a, int b) { return [b] { return 1; }(); }
lambda capture order | int f(int a, int b) { return [a, b] { return a - b; }(); } | int f(int a, int b) { return [b, a] { return a - b; }(); }
lambda init-capture | int f(int a) { return [y = a] { return y; }(); } | int f(int a) { return [y = a + 1] { return y; }(); }
lambda init-capture by reference | int f(int a) { return [y = a] { return y; }(); } | int f(int a) { return [&y = a] { return y; }(); }
lambda this vs *this | struct T { int m; int f(); }; int T::f() { return [this] { return m; }(); } | struct T { int m; int f(); }; int T::f() { return [*this] { return m; }(); }
lambda body | int f() { return [] { return 1; }(); } | int f() { return [] { return 2; }(); }
lambda parameter type | int f() { return [](int x) { return x; }(1); } | int f() { return [](long x) { return x; }(1); }
lambda mutable | int f(int a) { return [a]() { return a; }(); } | int f(int a) { return [a]() mutable { return a; }(); }
lambda noexcept | int f() { return [] { return 1; }(); } | int f() { return []() noexcept { return 1; }(); }
lambda consteval | int f() { return [] { return 1; }(); } | int f() { return []() consteval { return 1; }(); }
lambda static | int f() { return [] { return 1; }(); } | int f() { return []() static { return 1; }(); }
lambda generic | int f() { return [](int x) { return x; }(1); } | int f() { return [](auto x) { return x; }(1); }
lambda generic pack | int f() { return [](auto x) { return x; }(1); } | int f() { return [](auto... x) { return (x + ...); }(1); }
lambda nested body | int f(int a) { return [a] { return [a] { return a; }(); }(); } | int f(int a) { return [a] { return [a] { return a + 1; }(); }(); }
lambda nested capture | int f(int a) { return [a] { return [a] { return a; }(); }(); } | int f(int a) { return [a] { return [&a] { return a; }(); }(); }
lambda order | int f() { auto x = [] { return 1; }; auto y = [] { return 2; }; return x() - y(); } | int f() { auto x = [] { return 2; }; auto y = [] { return 1; }; return y() - x(); }
lambda in decltype | unsigned long f() { return sizeof(decltype([] { return 1; })); } | unsigned long f() { return sizeof(decltype([] { return 1L; })); }
local class member body | int f() { struct L { int g() { return 1; } }; return L().g(); } | int f() { struct L { int g() { return 2; } }; return L().g(); }
local class field type | int f() { struct L { int m; }; return sizeof(L); } | int f() { struct L { long m; }; return sizeof(L); }
local class field init | int f() { struct L { int m = 1; }; return L().m; } | int f() { struct L { int m = 2; }; return L().m; }
local class base | int f() { struct B { int m; }; struct L : B {}; return L().m; } | int f() { struct B { int m; }; struct L : virtual B {}; return L().m; }
local enum value | int f() { enum L { A = 1 }; return A; } | int f() { enum L { A = 2 }; return A; }
statement expression | int f() { return ({ 1; }); } | int f() { return ({ 2; }); }
label address | void *f() { x: y: return &&x; } | void *f() { x: y: return &&y; }
computed goto | void f(int a) { void *p = &&x; x: if (a--) goto *p; } | void f(int a) { void *p = &&x; x: y: if (a--) goto *&&y; }
choose expr | int f() { return __builtin_choose_expr(1, 2, 3); } | int f() { return __builtin_choose_expr(0, 2, 3); }
generic selection | int f(int a) { return _Generic(a, int: 1, default: 2); } | int f(int a) { return _Generic(a, int: 3, default: 2); }
generic selection type | int f(int a) { return _Generic(a, int: 1, default: 1); } | int f(long a) { return _Generic(a, int: 1, default: 1); }
GNU conditional | int f(int a, int b) { return a ?: b; } | int f(int a, int b) { return b ?: a; }
atomic order | int f(int *p) { return __atomic_load_n(p, 0); } | int f(int *p) { return __atomic_load_n(p, 2); }
atomic op | int f(int *p) { return __atomic_load_n(p, 0); } | int f(int *p) { return __atomic_fetch_add(p, 0, 0); }
shufflevector | typedef int V __attribute__((vector_size(16))); V f(V a) { return __builtin_shufflevector(a, a, 0, 1, 2, 3); } | typedef int V __attribute__((vector_size(16))); V f(V a) { return __builtin_shufflevector(a, a, 3, 2, 1, 0); }
convertvector | typedef int V __attribute__((vector_size(16))); typedef float W __attribute__((vector_size(16))); typedef unsigned U __attribute__((vector_size(16))); W f(V a) { return __builtin_convertvector(a, W); } | typedef int V __attribute__((vector_size(16))); typedef float W __attribute__((vector_size(16))); typedef unsigned U __attribute__((vector_size(16))); W f(V a) { return (W)__builtin_convertvector(a, U); }
ext vector accessor | typedef int V __attribute__((ext_vector_type(4))); int f(V a) { return a.x; } | typedef int V __attribute__((ext_vector_type(4))); int f(V a) { return a.y; }
offsetof | unsigned long f() { return __builtin_offsetof(S, m); } | unsigned long f() { return __builtin_offsetof(S, n); }
va_arg type | int f(int n, ...) { __builtin_va_list v; __builtin_va_start(v, n); int r = __builtin_va_arg(v, int); __builtin_va_end(v); return r; } | int f(int n, ...) { __builtin_va_list v; __builtin_va_start(v, n); int r = __builtin_va_arg(v, long); __builtin_va_end(v); return r; }
imaginary | _Complex double f() { return 1.0i; } | _Complex double f() { return 2.0i; }
asm template | void f() { asm("nop"); } | void f() { asm(""); }
asm volatile | int f() { int r; asm("" : "=r"(r)); return r; } | int f() { int r; asm volatile("" : "=r"(r)); return r; }
asm constraint | int f(int a) { int r; asm("" : "=r"(r) : "r"(a)); return r; } | int f(int a) { int r; asm("" : "=r"(r) : "m"(a)); return r; }
asm operand | int f(int a, int b) { int r; asm("" : "=r"(r) : "r"(a)); return r; } | int f(int a, int b) { int r; asm("" : "=r"(r) : "r"(b)); return r; }
asm clobber | void f() { asm("" ::: "memory"); } | void f() { asm("" ::: "cc"); }
"#;

const BINARY: &[&str] = &[
    "+", "-", "*", "/", "%", "&", "|", "^", "<<", ">>", "<", ">", "<=", ">=", "==", "!=", "&&",
    "||",
];
const UNARY: &[&str] = &["-a", "+a", "!a", "~a", "++a", "--a", "a++", "a--"];
const ASSIGN: &[&str] = &["=", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>="];

fn mutation_pairs() -> Vec<(String, String, String)> {
    let mut rows: Vec<(String, String, String)> = MUST_DIFFER
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let p: Vec<&str> = l.split(" | ").collect();
            assert_eq!(p.len(), 3, "row: {l}");
            (p[0].to_owned(), p[1].to_owned(), p[2].to_owned())
        })
        .collect();
    let all = |ops: &[&str], body: &dyn Fn(&str) -> String, rows: &mut Vec<_>| {
        for (i, x) in ops.iter().enumerate() {
            for y in &ops[i + 1..] {
                rows.push((format!("operator {x} vs {y}"), body(x), body(y)));
            }
        }
    };
    all(BINARY, &|op| format!("int f(int a, int b) {{ return a {op} b; }}"), &mut rows);
    all(UNARY, &|op| format!("int f(int a) {{ return {op}; }}"), &mut rows);
    all(ASSIGN, &|op| format!("void f(int &a, int b) {{ a {op} b; }}"), &mut rows);
    rows
}

#[test]
fn mutations_move_the_digest() {
    let mut bad = Vec::new();
    let rows = mutation_pairs();
    for (case, a, b) in &rows {
        let x = digest_in(&format!("{PRELUDE}{a}"), "f");
        let y = digest_in(&format!("{PRELUDE}{b}"), "f");
        if x.hash == y.hash {
            bad.push(case.clone());
        }
    }
    assert!(bad.is_empty(), "{} of {} mutations kept the digest: {bad:?}", bad.len(), rows.len());
}

// ---------------------------------------------------------------- (c)

/// The golden cases: a name and a definition of `f` (after [`PRELUDE`]).
const GOLDEN_CASES: &[(&str, &str)] = &[
    ("empty", "void f() {}"),
    ("literals", "long f() { return 16 + 'a' + 2u + (long)1.5 + true; }"),
    ("string", "const char *f() { return \"kiln\"; }"),
    ("placeholders", "const char *f(int &l) { l = __LINE__; return __FILE__; }"),
    ("operators", "int f(int a, int b) { a += b; return -a * (b << 2) % 3 ^ ~a; }"),
    ("calls", "int f(S &s, int a) { return g(a) + h(s.g(a)) + s.v() + s.S::v(); }"),
    ("members", "int f(S *p, S s) { return p->m + s.n + S::st; }"),
    ("enumerator", "int f() { return EA + EB; }"),
    (
        "casts",
        "long f(int a, void *p) { return static_cast<long>(a) + (long)p + reinterpret_cast<long>(p); }",
    ),
    ("locals", "int f(int a) { int x = a; static int y = 2; const long z(3); return x + y + z; }"),
    ("if", "int f(int a) { if (int b = a; b > 1) return 1; else if (a) return 2; return 3; }"),
    (
        "loops",
        "void f(int a) { while (a) --a; do ++a; while (a < 3); for (int i = 0; i < a; ++i) g(i); }",
    ),
    ("range-for", "int f(R r) { int s = 0; for (int x : r) s += x; return s; }"),
    (
        "switch",
        "int f(int a) { switch (a) { case 1: return 1; case 2: break; default: return 0; } return 3; }",
    ),
    ("goto", "void f(int a) { x: if (a--) goto x; }"),
    ("try", "int f() { try { return g(1); } catch (int e) { return e; } catch (...) { throw; } }"),
    ("conditional", "int f(int a, int b) { return a ? b : (a, b); }"),
    ("sizeof", "unsigned long f(int a) { return sizeof a + sizeof(long) + alignof(int); }"),
    ("noexcept", "int f() noexcept { return 1; }"),
    ("constexpr", "constexpr int f(int a) { return a; }"),
    ("method", "struct T { int m; int f() const &; }; int T::f() const & { return m; }"),
    ("new", "int *f() { return new int(1); }"),
    ("lambda", "int f(int a) { return [a, &b = a](int x) mutable { return x + a + b; }(1); }"),
    ("generic lambda", "int f() { return [](auto x) { return x; }(1); }"),
    (
        "local class",
        "int f() { struct L : S { int k = 2; int g() { return k; } }; enum Q { QA = 3 }; return L().g() + QA; }",
    ),
    (
        "gnu forms",
        "int f(int a, int *p) { return ({ a; }) + (a ?: 2) + __builtin_choose_expr(1, 3, 4) + __atomic_load_n(p, 0); }",
    ),
    (
        "asm",
        "int f(int a) { int r; asm volatile(\"mov %0, %1\" : \"=r\"(r) : \"r\"(a) : \"memory\"); return r; }",
    ),
    ("construct", "S f() { return S(); }"),
    ("init list", "int f() { int a[3] = {1}; int b[2]{1, 2}; return a[2] + b[0]; }"),
    ("new and delete", "void f(int n) { int *p = new int[n]{1, 2}; delete[] p; delete new S; }"),
    ("throw", "void f(int a) { if (a) throw a; throw; }"),
    ("traits", "bool f() { return __is_class(S) && !__is_union(S) && noexcept(g(1)); }"),
];

fn golden_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mason_def_hash.golden")
}

const GOLDEN_HEADER: &str = "\
# The definition hash's golden digests (kiln_abi::odr_def, scheme kiln-odr-1.d3).
# BUMP RULE (mortar O-11): a changed digest here means the serialization
# changed; bump ODR_DEF_SCHEME in crates/kiln_abi/src/odr_def/mod.rs (and
# spec/abi.md \"Definition hash\"), then regenerate with KILN_BLESS=1.
# case<TAB>scheme<TAB>digest<TAB>partial
";

#[test]
fn golden_digests() {
    let mut text = String::from(GOLDEN_HEADER);
    for &(case, src) in GOLDEN_CASES {
        let d = digest_in(&format!("{PRELUDE}{src}"), "f");
        writeln!(text, "{case}\t{ODR_DEF_SCHEME}\t{:032x}\t{}", d.hash, d.partial).unwrap();
    }
    let path = golden_path();
    if std::env::var_os("KILN_BLESS").is_some() {
        std::fs::write(&path, &text).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&path).unwrap_or_default();
    if want != text {
        let diff: Vec<String> = want
            .lines()
            .zip(text.lines())
            .filter(|(a, b)| a != b)
            .map(|(a, b)| format!("  - {a}\n  + {b}"))
            .collect();
        panic!(
            "the definition hash's serialization changed ({} lines differ):\n{}\n\
             mortar O-11: every change to what is hashed or how BUMPS the scheme: change \
             ODR_DEF_SCHEME (crates/kiln_abi/src/odr_def/mod.rs) and spec/abi.md, then \
             regenerate {} with KILN_BLESS=1",
            diff.len(),
            diff.join("\n"),
            path.display()
        );
    }
}

/// Every partial site writes its own token: two definitions skipped for
/// different reasons differ.
#[test]
fn partial_tokens_are_per_kind() {
    let a = digest_in(&format!("{PRELUDE}int f(int a = 1) {{ return a; }}"), "f");
    let b = digest_in(&format!("{PRELUDE}int f(int a = 2) {{ return a; }}"), "f");
    let c = digest_in(&format!("{PRELUDE}[[gnu::aligned(16)]] int f(int a) {{ return a; }}"), "f");
    assert!(a.partial && b.partial && c.partial);
    assert_eq!(a, b, "the same partial token for the same kind (subtree not hashed)");
    assert_ne!(a.hash, c.hash);
}

// ---------------------------------------------------------- deep trees

fn on_small_stack<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn_scoped(s, f)
            .unwrap()
            .join()
            .unwrap()
    })
}

fn on_big_stack<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|s| {
        std::thread::Builder::new().stack_size(1 << 30).spawn_scoped(s, f).unwrap().join().unwrap()
    })
}

/// 100,000-deep `a+a+...` and nested blocks: the parser gets a large
/// stack, the hasher a 256 KiB one.
#[test]
fn deep_trees_do_not_overflow() {
    const N: usize = 100_000;
    let mut sum = String::from("int f(int a) { return a");
    sum.push_str(&"+a".repeat(N));
    sum.push_str("; }");
    let mut blocks = String::from("int f(int a) ");
    blocks.push_str(&"{".repeat(N));
    blocks.push_str("++a;");
    blocks.push_str(&"}".repeat(N));
    blocks.push(' ');
    for src in [sum, blocks] {
        let (sm, tu) = on_big_stack(|| parse(&src));
        let ast = &tu.ast;
        let f = definitions(ast)[0];
        let d = on_small_stack(|| {
            let mut h = DefHasher::new(ast, &sm, AnonIds::new());
            let d = h.function(f);
            assert!(h.stats().nodes > N as u64);
            d
        });
        assert!(!d.partial);
        on_big_stack(move || drop(tu));
    }
}

// ---------------------------------------------------------- (d), (e)

const LIBRARY_UNIT_A: &str = "#include <algorithm>\n#include <string>\n#include <vector>\n\
#include <map>\n#include <functional>\n\
int use_a(int k) {\n  std::vector<int> v{3, 1, 2};\n  std::sort(v.begin(), v.end());\n\
  std::map<std::string, int> m;\n  m[\"a\"] = k;\n  std::string s = \"x\";\n  s += std::to_string(k);\n\
  std::function<int(int)> fn = [](int x) { return x + 1; };\n\
  return fn(v[0]) + int(m.size()) + int(s.size());\n}\n";

const LIBRARY_UNIT_B: &str = "#include <algorithm>\n#include <string>\n#include <vector>\n\
#include <map>\n#include <functional>\n\
long use_b(const std::string &s) {\n  std::map<std::string, int> m{{s, 1}};\n\
  std::vector<long> w(4, 2L);\n  std::reverse(w.begin(), w.end());\n  std::vector<int> v{1, 2};\n\
  std::sort(v.begin(), v.end(), std::greater<int>());\n  auto it = m.find(s);\n\
  return w[0] + v[1] + (it == m.end() ? 0 : it->second) + long(std::count(s.begin(), s.end(), 'a'));\n}\n\
int use_a2(int k) {\n  std::vector<int> v{3, 1, 2};\n  std::sort(v.begin(), v.end());\n  return v[0] + k;\n}\n";

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-def-hash-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// `src` preprocessed by kiln++ against `stdlib`, or `None` when that
/// library is not installed.
fn preprocess(src: &str, stdlib: &str, std: &str, tag: &str) -> Option<String> {
    use kiln_driver::{Io, SharedWriter, run};
    let dir = scratch(tag);
    let input = dir.join("u.cpp");
    let output = dir.join("u.ii");
    std::fs::write(&input, src).unwrap();
    let err = SharedWriter::new(std::io::sink());
    let io =
        Io { stdout: Box::new(std::io::sink()), stderr: err, stdin: Box::new(std::io::empty()) };
    let args: Vec<String> = [
        "-E",
        &format!("-std={std}"),
        &format!("-stdlib={stdlib}"),
        "-o",
        &output.display().to_string(),
        &input.display().to_string(),
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    let rc = run(&args, io);
    let text = std::fs::read_to_string(&output).ok();
    std::fs::remove_dir_all(&dir).ok();
    if rc != 0 {
        eprintln!("skipped: kiln++ -E -stdlib={stdlib} failed ({rc})");
        return None;
    }
    text
}

fn library_opts(std: Std) -> ParseOptions {
    ParseOptions { std, gnu: true, aligned_allocation: true, ..ParseOptions::default() }
}

/// The per-TU component of a mangled name or `<type>` encoding, if any
/// (an independent reader from refs.rs's guard).
fn per_tu_component(enc: &str) -> Option<&'static str> {
    let b = enc.as_bytes();
    let mut i = if enc.starts_with("_Z") { 2 } else { 0 };
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_digit() {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let n: usize = enc[start..i].parse().unwrap();
            let name = &enc[i..(i + n).min(enc.len())];
            if name.starts_with("_GLOBAL__N") {
                return Some("_GLOBAL__N");
            }
            if name.starts_with('$') {
                return Some("$_N");
            }
            i += n;
            continue;
        }
        match c {
            b'Z' => return Some("Z...E"),
            b'U' if matches!(b.get(i + 1), Some(b't' | b'l')) => return Some("Ut/Ul"),
            b'L' if b.get(i + 1) == Some(&b'_') && b.get(i + 2) == Some(&b'Z') => i += 2,
            b'L' if b.get(i + 1).is_some_and(u8::is_ascii_lowercase) => {
                while i < b.len() && b[i] != b'E' {
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

struct UnitHash {
    digests: BTreeMap<String, DefDigest>,
    names: Vec<String>,
    stats: kiln_abi::odr_def::DefStats,
}

fn hash_unit(ii: String) -> UnitHash {
    let (sm, tu) = parse_opts("u.ii", ii, &library_opts(Std::Cxx20));
    let ast = &tu.ast;
    let mut h = DefHasher::new(ast, &sm, AnonIds::new());
    h.collect_names();
    let mut anon = AnonIds::new();
    let mut digests = BTreeMap::new();
    for d in definitions(ast) {
        let name = mangle::mangle_decl(ast, &mut anon, d);
        let digest = h.function(d);
        // A definition whose own name is per-TU has no row (O-2).
        if per_tu_component(&name).is_none() {
            digests.insert(name, digest);
        }
    }
    UnitHash { digests, names: h.names().to_vec(), stats: h.stats().clone() }
}

fn report(stdlib: &str, u: &UnitHash) {
    let s = &u.stats;
    eprintln!(
        "{stdlib}: {} definitions hashed, {} partial ({:.1}%), {} nodes, {} bytes",
        s.definitions,
        s.partial_definitions,
        100.0 * s.partial_definitions as f64 / s.definitions.max(1) as f64,
        s.nodes,
        s.bytes
    );
    let mut by: BTreeMap<String, u64> = BTreeMap::new();
    for (&(area, site), &n) in &s.partial_sites {
        *by.entry(format!("{area:?} {}", site_name(site))).or_default() += n;
    }
    let mut by: Vec<_> = by.into_iter().collect();
    by.sort_by_key(|x| std::cmp::Reverse(x.1));
    for (k, n) in by.iter().take(25) {
        eprintln!(
            "  partial {k}: {n} ({:.1}% of definitions)",
            100.0 * *n as f64 / s.definitions.max(1) as f64
        );
    }
}

fn site_name(s: kiln_abi::odr_def::Site) -> String {
    use kiln_abi::odr_def::Site;
    match s {
        Site::Expr(k) => format!("expr {k:?}"),
        Site::Stmt(k) => format!("stmt {k:?}"),
        Site::Decl(k) => format!("decl {k:?}"),
        Site::Item(i) => format!("item {i:?}"),
    }
}

fn library_check(stdlib: &str) {
    let (Some(a), Some(b)) = (
        preprocess(LIBRARY_UNIT_A, stdlib, "c++20", &format!("{stdlib}-a")),
        preprocess(LIBRARY_UNIT_B, stdlib, "c++20", &format!("{stdlib}-b")),
    ) else {
        return;
    };
    let (ua, ub) = on_big_stack(|| (hash_unit(a), hash_unit(b)));
    // (d) no name fed to the hash has a per-TU component.
    for u in [&ua, &ub] {
        let bad: Vec<&String> = u.names.iter().filter(|n| per_tu_component(n).is_some()).collect();
        assert!(
            bad.is_empty(),
            "{stdlib}: per-TU names fed to the hash: {:?}",
            &bad[..bad.len().min(10)]
        );
        assert!(!u.names.is_empty());
    }
    // (e) the two units agree on every definition they share.
    let mut shared = 0;
    let mut bad = Vec::new();
    for (name, da) in &ua.digests {
        if let Some(db) = ub.digests.get(name) {
            shared += 1;
            if da != db {
                bad.push(name.clone());
            }
        }
    }
    report(stdlib, &ua);
    report(stdlib, &ub);
    eprintln!("{stdlib}: {shared} definitions in both units, {} disagree", bad.len());
    assert!(shared > 100, "{stdlib}: only {shared} shared definitions");
    assert!(
        bad.is_empty(),
        "{stdlib}: units disagree on {} definitions: {:?}",
        bad.len(),
        &bad[..bad.len().min(20)]
    );
}

#[test]
fn library_unit_libstdcxx() {
    library_check("libstdc++");
}

#[test]
fn library_unit_libcxx() {
    library_check("libc++");
}

// ---------------------------------------------------------------- cost

/// Deliverable 10: the cost of hashing every function definition of a
/// unit against parsing it. Run in release:
/// `cargo test --release -p kiln_codegen --test mason_def_hash -- --ignored --nocapture cost_report`.
#[test]
#[ignore = "a measurement, run by hand in release"]
fn cost_report() {
    let bench =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/frontend/format-like-g.cpp");
    let units = [
        (
            "format-like-g (libstdc++)",
            std::fs::read_to_string(&bench).unwrap(),
            "libstdc++",
            "c++26",
        ),
        ("library unit A (libstdc++)", LIBRARY_UNIT_A.to_owned(), "libstdc++", "c++20"),
        ("library unit A (libc++)", LIBRARY_UNIT_A.to_owned(), "libc++", "c++20"),
    ];
    for (name, src, stdlib, std) in units {
        let tp = std::time::Instant::now();
        let Some(ii) = preprocess(&src, stdlib, std, "cost") else { continue };
        let pp = tp.elapsed();
        let std = if std == "c++26" { Std::Cxx26 } else { Std::Cxx20 };
        on_big_stack(|| {
            let t0 = std::time::Instant::now();
            let (sm, tu) = parse_opts("u.ii", ii, &library_opts(std));
            let parse = t0.elapsed();
            let ast = &tu.ast;
            let defs = definitions(ast);
            let t1 = std::time::Instant::now();
            let mut h = DefHasher::new(ast, &sm, AnonIds::new());
            for &d in &defs {
                h.function(d);
            }
            let hash = t1.elapsed();
            let s = h.stats().clone();
            // Again with the memo maps warm: the walk without mangling.
            let t2 = std::time::Instant::now();
            for &d in &defs {
                h.function(d);
            }
            let warm = t2.elapsed();
            let front = pp + parse;
            eprintln!(
                "{name}: warm second pass {:.2} ms (mangling and the guard: the rest); \
                 preprocess+parse+sema {:.1} ms, hashing {:.2}% of it",
                warm.as_secs_f64() * 1e3,
                front.as_secs_f64() * 1e3,
                100.0 * hash.as_secs_f64() / front.as_secs_f64()
            );
            let rate = |hit: u64, miss: u64| 100.0 * hit as f64 / (hit + miss).max(1) as f64;
            eprintln!(
                "{name}: parse+sema {:.1} ms; hashing {:.2} ms ({:.2}%) for {} definitions, {} nodes \
                 ({:.0} ns/node), {} bytes; entity memo {:.1}% hits ({} mangled), type memo {:.1}% \
                 hits ({} mangled); hasher memory {} KiB; partial {}",
                parse.as_secs_f64() * 1e3,
                hash.as_secs_f64() * 1e3,
                100.0 * hash.as_secs_f64() / parse.as_secs_f64(),
                s.definitions,
                s.nodes,
                hash.as_nanos() as f64 / s.nodes.max(1) as f64,
                s.bytes,
                rate(s.entity_hits, s.entity_misses),
                s.entity_misses,
                rate(s.type_hits, s.type_misses),
                s.type_misses,
                h.memory() / 1024,
                s.partial_definitions,
            );
        });
    }
}

// ------------------------------------------------- K-3c (WP-1583): init

/// The definition picked by `pick` in `src` parsed at `std`: a function
/// definition (by [`DefHasher::function`]) or a variable definition (by
/// [`DefHasher::variable`]).
fn digest_pick(src: &str, std: Std, pick: &dyn Fn(&Ast, DeclId) -> bool) -> DefDigest {
    let opts = ParseOptions { std, ..ParseOptions::default() };
    let (sm, tu) = parse_opts("t.cpp", src.to_owned(), &opts);
    let ast = &tu.ast;
    let mut h = DefHasher::new(ast, &sm, AnonIds::new());
    let funcs: Vec<DeclId> = definitions(ast).into_iter().filter(|&d| pick(ast, d)).collect();
    if let [f] = funcs[..] {
        return h.function(f);
    }
    assert!(funcs.is_empty(), "several definitions picked");
    let mut vars: Vec<DeclId> = ast
        .decls
        .ids()
        .filter(|&d| ast.decls.var(d).is_some() && pick(ast, d))
        .map(|d| ast.first(d))
        .collect();
    vars.sort();
    vars.dedup();
    assert_eq!(vars.len(), 1, "one definition picked");
    // As the rows do: the definition of the first declaration.
    h.variable(ast.definition(vars[0]).unwrap_or(vars[0]))
}

/// The function or variable named `name`.
fn named(name: &'static str) -> impl Fn(&Ast, DeclId) -> bool {
    move |ast, d| ident(ast, d) == name
}

/// The constructor (`kind` `CXXConstructor`) or destructor of class
/// `class` with `params` parameters.
fn structor(
    class: &'static str,
    kind: kiln_ast::decl::DeclKind,
    params: usize,
) -> impl Fn(&Ast, DeclId) -> bool {
    move |ast, d| {
        let n = ast.decls.get(d);
        n.kind() == kind
            && n.ctx().is_some_and(|c| ident(ast, c) == class)
            && ast.decls.function(d).is_some_and(|f| ast.lists.len(f.params) == params)
    }
}

/// What the init rows below see besides [`PRELUDE`].
const INIT_PRELUDE: &str = "namespace std { class type_info { public: virtual ~type_info(); \
     const char *name() const; };\n\
     template <class E> class initializer_list { const E *b; decltype(sizeof 0) n; \
     public: constexpr initializer_list() : b(nullptr), n(0) {} }; }\n\
     void *operator new(decltype(sizeof 0), void *) noexcept;\n\
     struct P { int x; int y; };\nunion U { int i; float f; };\n\
     struct C { C(); C(int); C(long); C(const C &); ~C(); int v = 1; };\n\
     struct L { L(std::initializer_list<int>); };\n";

/// Rows that must differ (after [`PRELUDE`] and [`INIT_PRELUDE`]), one a
/// line: a case, two definitions of `f`.
const MUST_DIFFER_INIT: &str = r"
constructor selected | C f() { return C(1); } | C f() { return C(1L); }
constructor argument | C f() { return C(1); } | C f() { return C(2); }
construct vs default | int f() { C c(1); return c.v; } | int f() { C c; return c.v; }
copy kept | C f(C &c) { C d(c); return d; } | C f(C &c) { C d(1); return d; }
new value | int *f() { return new int(1); } | int *f() { return new int(2); }
new array bound | int *f() { return new int[2]; } | int *f() { return new int[3]; }
new init | int *f() { return new int; } | int *f() { return new int(); }
new placement | int *f(void *p) { return new (p) int(1); } | int *f(void *p) { return new int(1); }
new global | C *f() { return new C; } | C *f() { return ::new C; }
new parens vs braces | C *f() { return new C(1); } | C *f() { return new C{1}; }
delete form | void f(int *p) { delete p; } | void f(int *p) { delete[] p; }
throw value | void f() { throw 1; } | void f() { throw 2; }
throw vs rethrow | void f() { throw 1; } | void f() { throw; }
typeid type | const std::type_info &f() { return typeid(int); } | const std::type_info &f() { return typeid(long); }
typeid type vs expr | const std::type_info &f(int a) { return typeid(int); } | const std::type_info &f(int a) { return typeid(a); }
trait kind | bool f() { return __is_class(P); } | bool f() { return __is_union(P); }
trait type | bool f() { return __is_class(P); } | bool f() { return __is_class(U); }
array trait | unsigned long f() { return __array_extent(int[2][3], 1); } | unsigned long f() { return __array_extent(int[2][4], 1); }
noexcept operand | bool f() { return noexcept(g(1)); } | bool f() { return noexcept(h(1)); }
init list order | int f() { P p{1, 2}; return p.x; } | int f() { P p{2, 1}; return p.x; }
init list size | int f() { P p{1}; return p.x; } | int f() { P p{1, 0}; return p.x; }
designated field | int f() { P p{.y = 1}; return p.x; } | int f() { P p{.x = 1}; return p.x; }
union member | int f() { U u{.i = 1}; return u.i; } | int f() { U u{.f = 1}; return u.i; }
array filler | int f() { int a[3] = {1}; return a[0]; } | int f() { int a[3] = {2}; return a[0]; }
value init | int f() { return int(); } | int f() { return long(); }
braces vs parens | int f() { P p{1, 2}; return p.x; } | int f() { P p(1, 2); return p.x; }
initializer_list | void f() { L l{1, 2}; } | void f() { L l{1, 3}; }
compound literal | int f() { return (P){1, 2}.x; } | int f() { return (P){2, 1}.x; }
";

/// Deliverable 9: every init kind's operands move the digest.
#[test]
fn init_mutations_move_the_digest() {
    let mut bad = Vec::new();
    for l in MUST_DIFFER_INIT.lines().filter(|l| !l.trim().is_empty()) {
        let p: Vec<&str> = l.split(" | ").collect();
        assert_eq!(p.len(), 3, "row: {l}");
        let x = digest_pick(&format!("{PRELUDE}{INIT_PRELUDE}{}", p[1]), Std::Cxx20, &named("f"));
        let y = digest_pick(&format!("{PRELUDE}{INIT_PRELUDE}{}", p[2]), Std::Cxx20, &named("f"));
        assert!(!x.partial && !y.partial, "{}: partial", p[0]);
        if x.hash == y.hash {
            bad.push(p[0]);
        }
    }
    assert!(bad.is_empty(), "init mutations kept the digest: {bad:?}");
}

/// Pairs (after both preludes) that must hash alike: elidable copies,
/// parentheses, designators by resolved field.
const MUST_EQUAL_INIT: &[(&str, &str, &str)] = &[
    ("parenthesized construct", "C f() { return (C(1)); }", "C f() { return C(1); }"),
    (
        "designated by resolved field",
        "int f() { P p{.x = 1, .y = 2}; return p.x; }",
        "int f() { P p{1, 2}; return p.x; }",
    ),
];

#[test]
fn init_must_equal_pairs() {
    for &(case, a, b) in MUST_EQUAL_INIT {
        let x = digest_pick(&format!("{PRELUDE}{INIT_PRELUDE}{a}"), Std::Cxx20, &named("f"));
        let y = digest_pick(&format!("{PRELUDE}{INIT_PRELUDE}{b}"), Std::Cxx20, &named("f"));
        assert_eq!(x, y, "{case}");
    }
}

/// Elidable copies are transparent: `return T();` and `T x = T(1);` at
/// C++14 (a move from a temporary, elided) equal C++17 (no copy).
#[test]
fn elidable_copies_across_standards() {
    let src = "struct T { T(); T(int); T(const T &); T(T &&); int m; };\n\
               T f() { return T(); }\nint g() { T x = T(1); return x.m; }\n\
               T h(T t) { return T(t); }\n";
    for name in ["f", "g", "h"] {
        let a = digest_pick(src, Std::Cxx14, &named(name));
        let b = digest_pick(src, Std::Cxx17, &named(name));
        assert!(!a.partial, "{name}");
        assert_eq!(a, b, "{name}");
    }
}

/// O-13 K1: a default argument is hashed at the use, so the caller
/// differs when the default does; equal when it is the same.
#[test]
fn default_argument_at_the_use() {
    let a = digest_pick("int d(int a = 1);\nint f() { return d(); }", Std::Cxx20, &named("f"));
    let b = digest_pick("int d(int a = 2);\nint f() { return d(); }", Std::Cxx20, &named("f"));
    let c = digest_pick("int d(int = 1);\n\nint f() { return d(); }", Std::Cxx20, &named("f"));
    assert!(!a.partial);
    assert_ne!(a.hash, b.hash);
    assert_eq!(a, c);
}

/// O-13 K2: a default member initializer is hashed at the constructor
/// that uses it (here the implicit default constructor, synthesized).
#[test]
fn default_member_initializer_at_the_use() {
    use kiln_ast::decl::DeclKind::CXXConstructor;
    let use_it = "struct N { int v = 1; };\nint f() { N n; return n.v; }\n";
    let a = digest_pick(use_it, Std::Cxx20, &structor("N", CXXConstructor, 0));
    let b =
        digest_pick(&use_it.replace("= 1", "= 2"), Std::Cxx20, &structor("N", CXXConstructor, 0));
    assert!(!a.partial);
    assert_ne!(a.hash, b.hash);
}

/// Constructors: the ctor-initializers in initialization order with
/// their targets, then the body.
#[test]
fn constructor_initializers() {
    use kiln_ast::decl::DeclKind::CXXConstructor;
    let k = |src: &str| {
        digest_pick(
            &format!("struct B {{ B(int); }};\nstruct V {{ V(int); }};\n{src}"),
            Std::Cxx20,
            &structor("X", CXXConstructor, 1),
        )
    };
    let base = k("struct X : B { int a, b; X(int); };\nX::X(int i) : B(i), a(1), b(2) {}");
    for other in [
        "struct X : B { int a, b; X(int); };\nX::X(int i) : B(i), a(2), b(2) {}",
        "struct X : B { int a, b; X(int); };\nX::X(int i) : B(1), a(1), b(2) {}",
        "struct X : B { int a, b; X(int); };\nX::X(int i) : B(i), a(1), b(2) { a = 3; }",
        "struct X : B { int b, a; X(int); };\nX::X(int i) : B(i), a(1), b(2) {}",
        "struct X : virtual V, B { int a, b; X(int); };\nX::X(int i) : V(i), B(i), a(1), b(2) {}",
        "struct X : B { int a, b; X(int); X(int, int); };\nX::X(int i) : X(i, i) {}\nX::X(int, int) : B(0) {}",
    ] {
        let d = k(other);
        assert!(!d.partial, "{other}");
        assert_ne!(base.hash, d.hash, "{other}");
    }
    assert!(!base.partial);
}

/// A virtual destructor hashes the deallocation function it selects.
#[test]
fn destructor_deallocation_function() {
    use kiln_ast::decl::DeclKind::CXXDestructor;
    let a = "struct Y { virtual ~Y(); };\nY::~Y() {}\n";
    let b = "struct Y { virtual ~Y(); void operator delete(void *); };\nY::~Y() {}\n";
    let x = digest_pick(a, Std::Cxx20, &structor("Y", CXXDestructor, 0));
    let y = digest_pick(b, Std::Cxx20, &structor("Y", CXXDestructor, 0));
    assert!(!x.partial && !y.partial);
    assert_ne!(x.hash, y.hash);
}

/// A predicate picking one declaration.
type Pick = Box<dyn Fn(&Ast, DeclId) -> bool>;

/// Synthesized members: not `partial`, and not moved by sema state a unit
/// computes only when it needs it (the exception specification, a
/// constant evaluation).
#[test]
fn synthesized_members_ignore_lazy_state() {
    use kiln_ast::decl::DeclKind::{CXXConstructor, CXXDestructor, CXXMethod};
    let class = "struct M { M(); M(const M &); M &operator=(const M &); ~M(); };\n\
                 struct X { M m; int i = 4; };\n";
    let plain = format!("{class}void f(X &a) {{ X b; X c(b); a = c; }}\n");
    let lazy = format!(
        "{class}void f(X &a) {{ X b; X c(b); a = c; }}\n\
         static_assert(!noexcept(X()) && !noexcept(X(X())));\n"
    );
    let picks: [(&str, Pick); 4] = [
        ("default ctor", Box::new(structor("X", CXXConstructor, 0))),
        ("copy ctor", Box::new(structor("X", CXXConstructor, 1))),
        ("dtor", Box::new(structor("X", CXXDestructor, 0))),
        (
            "assign",
            Box::new(|ast: &Ast, d: DeclId| {
                ast.decls.get(d).kind() == CXXMethod
                    && ast.decls.get(d).ctx().is_some_and(|c| ident(ast, c) == "X")
            }),
        ),
    ];
    for (what, pick) in &picks {
        let a = digest_pick(&plain, Std::Cxx20, &**pick);
        let b = digest_pick(&lazy, Std::Cxx20, &**pick);
        assert!(!a.partial, "{what}");
        assert_eq!(a, b, "{what}");
    }
}

/// Variables: the initializer as an expression, never its value (K9);
/// constant and dynamic initialization of one expression alike; N22's
/// out-of-line definition (C++14) equals the inline one (C++17).
#[test]
fn variable_initializers() {
    let var = |src: &str| digest_pick(src, Std::Cxx20, &named("v"));
    let one = var("inline int v = 2;");
    assert!(!one.partial);
    assert_ne!(one.hash, var("inline int v = 1 + 1;").hash, "K9: the expression, not the value");
    assert_ne!(one.hash, var("inline long v = 2;").hash);
    assert_ne!(one.hash, var("inline thread_local int v = 2;").hash);
    assert_eq!(
        var("constexpr int k() { return 1; }\ninline int v = k();"),
        var("int k();\ninline int v = k();"),
        "constant and dynamic initialization of one expression"
    );
    let n22 = "struct A { static constexpr int v = 5; };\n";
    let a = digest_pick(&format!("{n22}constexpr int A::v;\n"), Std::Cxx14, &named("v"));
    let b = digest_pick(n22, Std::Cxx17, &named("v"));
    assert!(!a.partial);
    assert_eq!(a, b, "N22");
    let t =
        "template <class T> struct Q { static inline int v = sizeof(T); };\nint u = Q<int>::v;\n";
    let q = digest_pick(t, Std::Cxx20, &|ast: &Ast, d: DeclId| {
        ident(ast, d) == "v" && !is_templated_def(ast, d)
    });
    assert!(!q.partial);
}

/// Deliverable 4 (mortar's request): parentheses are transparent, so
/// these pairs must differ through type, value category or callee.
#[test]
fn parentheses_pairs_differ_by_meaning() {
    let f = |src: &str| digest_pick(src, Std::Cxx20, &named("f"));
    let pairs = [
        (
            "decltype((x)) vs decltype(x)",
            "int gx;\nlong f() { decltype((gx)) r = gx; return r; }",
            "int gx;\nlong f() { decltype(gx) r = gx; return r; }",
        ),
        (
            "return (x) vs return x in decltype(auto)",
            "int gx;\ndecltype(auto) f() { return (gx); }",
            "int gx;\ndecltype(auto) f() { return gx; }",
        ),
        (
            "(q)(a) vs q(a) with argument-dependent lookup",
            "namespace n { struct T {}; int q(T); }\ntemplate <class U> int q(U);\n\
             int f(n::T a) { return (q)(a); }",
            "namespace n { struct T {}; int q(T); }\ntemplate <class U> int q(U);\n\
             int f(n::T a) { return q(a); }",
        ),
    ];
    for (case, a, b) in pairs {
        let (x, y) = (f(a), f(b));
        assert!(!x.partial && !y.partial, "{case}");
        assert_ne!(x.hash, y.hash, "{case}: hashed equal");
    }
}
