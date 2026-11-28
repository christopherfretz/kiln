//! Written extents of expressions (spec/ast.md §21): every kind of §21.2
//! has the extent of its text, the nodes with nothing written have none,
//! and macro locations map by the rule of `kiln_lex::token_range`.

use std::sync::Arc;

use kiln_ast::dump::written_exprs;
use kiln_ast::expr_extent::WrittenExtents;
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_lex::LexOptions;
use kiln_parse::{ConstexprLimits, ExprExtents, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

const SRC: &str = r#"
#define ID(x) x
#define PAREN(x) (x)
#define ONE 1
#define CAT(a, b) a##b
#define PLUS(a, b) a + b
#define ADD1(x) x + 1
namespace n { int v = 0; template <class T> T tv = T(); }
struct S {
  int x = 4;
  int m(int a, int b = 2) { return x + a * b; }
  S(int v) : x(v) {}
  S() = default;
  bool operator==(const S &o) const { return x == o.x; }
  S operator+(const S &o) const { return S(x + o.x); }
  int operator[](int i) const { return i; }
  operator int() const { return x; }
  static int sm() { return 3; }
  int m2() const { return this->x; }
};
unsigned long long operator""_k(unsigned long long v) { return v; }
int f(int a, int b = 3) { return a - b; }
template <class... Ts> int sum(Ts... ts) { return (ts + ... + 0) + sizeof...(Ts); }
template <class T> concept Small = sizeof(T) < 8;
template <class... Ts> int fwd(Ts... ts) { return sum(ts...) + ts...[0]; }
typedef float f2 __attribute__((ext_vector_type(2)));
int user(S s, S t, int *p) {
  int arr[3] = {1, 2, 3};
  const char *str = "ab" "cd";
  int k = f(1) + s.m(arr[1]) + sizeof(int) + static_cast<int>(1.5) + (int)2.5;
  k += -k;
  k++;
  bool ne = s != t;
  S u = s + t;
  int ix = s[k];
  int conv = s;
  k = k ? k : n::v;
  k = ID(k + 1) + PAREN(k) + ONE + CAT(k, );
  int pl = PLUS(k, 2);
  int a1 = ADD1(k * 3);
  auto l = [&] { return k; };
  int *q = new int(3);
  delete q;
  unsigned long long lit = 12_k;
  int tv = n::tv<int>;
  int st = S::sm();
  int pk = (k);
  int hb = __has_builtin(__builtin_expect);
  bool sm = Small<int>;
  int bc = __builtin_bit_cast(int, 1.0f) + __builtin_choose_expr(1, 2, 3) + __builtin_LINE();
  unsigned long rk = __array_rank(int[2][3]) + __is_lvalue_expr(k);
  f2 v2 = {1.0f, 2.0f};
  float vx = v2.x;
  int so = sizeof(k) + sizeof k;
  return k + l() + *q + int(lit) + tv + st + pk + pl + a1 + hb + sm + bc + int(rk) + int(vx) + so + fwd(1, 2) + noexcept(f(1)) + alignof(int) + sum(1, 2);
}
"#;

struct Parsed {
    sm: SourceManager,
    interner: Arc<Interner>,
    tu: kiln_parse::ParsedTu,
}

fn parse() -> Parsed {
    parse_src(SRC)
}

fn parse_src(src: &str) -> Parsed {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let opts = ParseOptions::default();
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    assert_eq!(engine.error_count(), 0, "the source has errors");
    drop(engine);
    Parsed { sm, interner, tu }
}

/// Every expression with an extent, as `(kind, text)`, and the kinds of
/// those without one.
fn extents(p: &Parsed) -> (Vec<(String, String)>, Vec<String>) {
    let x = ExprExtents::new(
        &p.tu.ast,
        &p.sm,
        &p.interner,
        LexOptions::new(ParseOptions::default().std),
    );
    let mut some = Vec::new();
    let mut none = Vec::new();
    for e in written_exprs(&p.tu.ast) {
        let kind = format!("{:?}", p.tu.ast.exprs.get(e).kind());
        if let Some((f, b, end)) = x.written_range(e) {
            assert!(x.written_extent(e).is_some(), "{kind}: a range is an extent");
            let text = &p.sm.text(f)[b as usize..end as usize];
            some.push((kind, text.to_owned()));
        } else {
            assert!(x.written_extent(e).is_none(), "{kind}: an extent is a range");
            none.push(kind);
        }
    }
    (some, none)
}

#[test]
fn every_kind_has_its_text() {
    let p = parse();
    let (some, none) = extents(&p);
    let has = |kind: &str, text: &str| {
        assert!(
            some.iter().any(|(k, t)| k == kind && t == text),
            "no {kind} `{text}` among {:?}",
            some.iter().filter(|(k, _)| k == kind).collect::<Vec<_>>()
        );
    };
    has("DeclRefExpr", "k");
    has("DeclRefExpr", "n::v");
    has("IntegerLiteral", "1");
    has("StringLiteral", r#""ab" "cd""#);
    has("CallExpr", "f(1)");
    has("CXXMemberCallExpr", "s.m(arr[1])");
    has("ArraySubscriptExpr", "arr[1]");
    has("UnaryExprOrTypeTraitExpr", "sizeof(int)");
    has("UnaryExprOrTypeTraitExpr", "alignof(int)");
    has("UnaryExprOrTypeTraitExpr", "sizeof(k)");
    has("UnaryExprOrTypeTraitExpr", "sizeof k");
    has("CXXStaticCastExpr", "static_cast<int>(1.5)");
    has("CStyleCastExpr", "(int)2.5");
    has("CXXFunctionalCastExpr", "int(lit)");
    has("CompoundAssignOperator", "k += -k");
    has("UnaryOperator", "-k");
    has("UnaryOperator", "k++");
    has("UnaryOperator", "*q");
    has("CXXRewrittenBinaryOperator", "s != t");
    has("CXXOperatorCallExpr", "s + t");
    has("CXXOperatorCallExpr", "s[k]");
    has("ConditionalOperator", "k ? k : n::v");
    has("LambdaExpr", "[&] { return k; }");
    has("CXXOperatorCallExpr", "l()");
    has("CXXNewExpr", "new int(3)");
    has("CXXDeleteExpr", "delete q");
    has("InitListExpr", "{1, 2, 3}");
    has("CXXFoldExpr", "(ts + ... + 0)");
    has("SizeOfPackExpr", "sizeof...(Ts)");
    has("CXXNoexceptExpr", "noexcept(f(1))");
    has("CXXThisExpr", "this");
    has("ParenExpr", "(k)");
    has("PackExpansionExpr", "ts...");
    has("PackIndexingExpr", "ts...[0]");
    has("ConceptSpecializationExpr", "Small<int>");
    has("BuiltinBitCastExpr", "__builtin_bit_cast(int, 1.0f)");
    has("ChooseExpr", "__builtin_choose_expr(1, 2, 3)");
    has("SourceLocExpr", "__builtin_LINE()");
    has("ArrayTypeTraitExpr", "__array_rank(int[2][3])");
    has("ExpressionTraitExpr", "__is_lvalue_expr(k)");
    has("ExtVectorElementExpr", "v2.x");
    // A user-defined literal is a call: the literal token.
    has("CallExpr", "12_k");
    // An implicit conversion function call: the object.
    has("CXXMemberCallExpr", "s");
    // An implicit `this` member access begins at the member's name.
    has("MemberExpr", "x");
    // Macros: an argument maps to its spelling, a whole invocation to
    // itself (also when the body is the argument), a parenthesized body to
    // the invocation.
    has("BinaryOperator", "k * 3");
    has("BinaryOperator", "ID(k + 1)");
    has("BinaryOperator", "ADD1(k * 3)");
    has("ParenExpr", "PAREN(k)");
    has("IntegerLiteral", "ONE");
    has("BinaryOperator", "PLUS(k, 2)");
    // A builtin macro's result is its whole invocation.
    has("IntegerLiteral", "__has_builtin(__builtin_expect)");
    // Nothing written: a default argument, a default member initializer
    // at its use, an implicit `this`.
    assert!(none.iter().any(|k| k == "CXXDefaultArgExpr"));
    assert!(!some.iter().any(|(k, _)| k == "CXXDefaultArgExpr" || k == "CXXDefaultInitExpr"));
    // An implicit capture's initializer is not written; the use is.
    assert!(!some.iter().any(|(k, t)| k == "DeclRefExpr" && t == "&"));
    has("DeclRefExpr", "k");
    // A pasted token's spelling is a scratch buffer: no extent.
    assert!(!some.iter().any(|(_, t)| t.contains("CAT")));
}

/// A `Some` of the tree's own extent is a pair of token starts; the
/// client's view drops exactly the pairs that are no file range.
#[test]
fn client_extents_are_the_mappable_tree_extents() {
    let p = parse();
    let x = ExprExtents::new(
        &p.tu.ast,
        &p.sm,
        &p.interner,
        LexOptions::new(ParseOptions::default().std),
    );
    let tree = WrittenExtents::new(&p.tu.ast);
    let mut dropped = 0;
    for e in written_exprs(&p.tu.ast) {
        match (tree.get(e), x.written_extent(e)) {
            (Some(a), Some(b)) => assert_eq!(a, b),
            (Some(_), None) => dropped += 1,
            (None, Some(_)) => panic!("an extent the tree does not have"),
            (None, None) => {}
        }
    }
    // The pasted `CAT(k, )` and the parts of macro bodies.
    assert!(dropped > 0);
}

#[test]
fn the_dump_has_a_line_per_expression() {
    let p = parse();
    let x = ExprExtents::new(
        &p.tu.ast,
        &p.sm,
        &p.interner,
        LexOptions::new(ParseOptions::default().std),
    );
    let dump = x.dump();
    assert_eq!(dump.lines().count(), written_exprs(&p.tu.ast).len());
    for line in dump.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        assert_eq!(fields.len(), 4, "{line}");
        assert_ne!(fields[1], "!", "{line}");
        if fields[3] != "-" {
            let (begin, end): (u32, u32) = (fields[2].parse().unwrap(), fields[3].parse().unwrap());
            assert!(begin < end && fields[1] == "t.cpp", "{line}");
        }
    }
    assert!(dump.contains("CXXDefaultArgExpr\t-\t-\t-\n"));
}

/// Names, destroyed types, designators and parenthesized aggregate
/// initializations whose ends or begins sema records (spec/ast.md
/// §21.3): dependent template-ids end at their `>`, operator names at
/// their last token, a qualifier through an implicit `this` and of a
/// concept-id begins the name, a pseudo-destructor ends at its destroyed
/// type, a designated clause begins at its `.` or `[`, a parenthesized
/// aggregate initialization at its `(` or its cast, and the value of a
/// non-type template argument is the parameter's name.
const NAMES: &str = r"
namespace ns { template <class T> concept C = sizeof(T) > 0; }
struct B { int f; int g() { return 0; } };
struct D : B { int h() { return B::f + B::g(); } };
template <class T> struct X : T {
  template <class U> int f() { return 0; }
  int k() { return this->template f<T>() + T::template v<int>; }
};
struct G { template <int N> int get() { return N; } };
template <class T> struct E : G {
  int g() { return G::get<0>() + get<1>() + this->get<2>(); }
};
struct R {};
bool operator==(R, R) { return true; }
struct Q {
  bool operator==(const Q &) const { return true; }
  template <int N> int operator()() const { return N; }
};
bool eq(Q a, Q b, R c) { return operator==(c, c) || a.operator==(b); }
int call(Q q) { return q.operator()<1>(); }
template <class T> bool teq(T a, T b) { return operator==(a, b); }
template <class T> int tcall(T q) { return q.template operator()<1>(); }
template <class T> requires ns::C<T> void cc(T) {}
template <class T> void pd(T *p, T x) { p->~T(); x.~decltype(x)(); }
void pd2(int *p) { typedef int I; p->~I(); }
struct A { int a, b; };
template <class T> T des() { return T{.a = 1, [0] = 2}; }
template <class T> T old() { return T{a: 1}; }
struct M { A m; M() : m(1, 2) {} };
A *pn() { return new A(1, 2); }
int pl() {
  A s(1, 2);
  int arr[2](1, 2);
  auto r = static_cast<int[3]>(42);
  A x = static_cast<A>(1);
  A y = A(1, 2);
  A z = (A)3;
  A w = A(5);
  return s.a + arr[0] + r[0] + x.a + y.b + z.a + w.a;
}
template <int N> struct V { int x{N}; };
V<3> v3{};
";

#[test]
fn recorded_ends_and_begins_have_their_text() {
    let p = parse_src(NAMES);
    let (some, none) = extents(&p);
    let has = |kind: &str, text: &str| {
        assert!(
            some.iter().any(|(k, t)| k == kind && t == text),
            "no {kind} `{text}` among {:?}",
            some.iter().filter(|(k, _)| k == kind).collect::<Vec<_>>()
        );
    };
    // WP-2350: a dependent template-id's `>`, an operator name's end.
    has("CXXDependentScopeMemberExpr", "this->template f<T>");
    has("DependentScopeDeclRefExpr", "T::template v<int>");
    has("DeclRefExpr", "operator==");
    has("MemberExpr", "a.operator==");
    has("MemberExpr", "q.operator()<1>");
    has("UnresolvedLookupExpr", "operator==");
    has("CXXDependentScopeMemberExpr", "q.template operator()<1>");
    // A template-id through an implicit (dependent) `this`.
    has("CXXDependentScopeMemberExpr", "G::get<0>");
    has("CXXDependentScopeMemberExpr", "get<1>");
    has("CXXDependentScopeMemberExpr", "this->get<2>");
    // WP-2351: a qualifier through an implicit `this`, of a concept-id.
    has("MemberExpr", "B::f");
    has("MemberExpr", "B::g");
    has("ConceptSpecializationExpr", "ns::C<T>");
    // WP-2352: a destroyed type's end, a designator, the parentheses.
    has("CXXPseudoDestructorExpr", "p->~T");
    has("CXXPseudoDestructorExpr", "x.~decltype(x)");
    has("CXXPseudoDestructorExpr", "p->~I");
    has("DesignatedInitExpr", ".a = 1");
    has("DesignatedInitExpr", "[0] = 2");
    has("DesignatedInitExpr", "a: 1");
    has("CXXParenListInitExpr", "(1, 2)");
    has("CXXParenListInitExpr", "static_cast<int[3]>(42)");
    has("CXXParenListInitExpr", "static_cast<A>(1)");
    has("CXXParenListInitExpr", "(A)3");
    // A functional cast of one operand begins it at the type, as clang.
    has("CXXParenListInitExpr", "A(5)");
    // WP-2353: the argument's value at the parameter's name.
    has("SubstNonTypeTemplateParmExpr", "N");
    for k in [
        "CXXPseudoDestructorExpr",
        "DesignatedInitExpr",
        "CXXParenListInitExpr",
        "SubstNonTypeTemplateParmExpr",
    ] {
        assert!(!none.iter().any(|n| n == k), "a {k} without an extent");
    }
    let parens = some.iter().filter(|(k, _)| k == "CXXParenListInitExpr").count();
    // `m(1, 2)`, `new A(1, 2)`, `s(1, 2)`, `arr[2](1, 2)`, `A(1, 2)` and
    // the four casts.
    assert_eq!(parens, 9, "{some:?}");
}

/// The five shapes WP-2350 left without a begin or an end (WP-2460..
/// WP-2464): an implicit `this->operator T*`, a member named by a
/// dependent using-declarator, `&B::f` of an overloaded member and a
/// static member template's specialization through an implicit `this`, an
/// array `new`'s parenthesized aggregate initialization, and
/// `&this->sf<int>`.
const LEFT: &str = r"
template <class T> struct C {
  operator T*() const;
  T* g() const { return operator T*(); }
};
template <class T> struct U : T {
  using T::f;
  int h() { return f(); }
};
struct B {
  int f();
  int f(int);
  template <int N> static int s(int);
  template <int N> int s(long);
};
struct D : B {
  int h() { int (B::*q)() = &B::f; return s<0>(0) + B::s<1>(0) + (this->*q)(); }
};
struct S { template <class V> static int sf(); };
template <class T> struct X : S { auto g() { return &this->sf<int>; } };
struct Y : S { auto g() { return &this->sf<int>; } };
struct R { int v; };
int *pn() { return new int[2](1, 2); }
int *pu() { return new int[](1, 2, 3); }
R (*pr())[2] { return new R[2][2]({R(4)}); }
";

#[test]
fn the_left_shapes_have_their_text() {
    let p = parse_src(LEFT);
    let (some, _) = extents(&p);
    let count =
        |kind: &str, text: &str| some.iter().filter(|(k, t)| k == kind && t == text).count();
    let has = |kind: &str, text: &str| {
        assert!(
            count(kind, text) > 0,
            "no {kind} `{text}` among {:?}",
            some.iter().filter(|(k, _)| k == kind).collect::<Vec<_>>()
        );
    };
    // WP-2460: the conversion-type-id's last token ends the name.
    has("CXXDependentScopeMemberExpr", "operator T*");
    has("CallExpr", "operator T*()");
    // WP-2461: the written name begins it.
    has("CXXDependentScopeMemberExpr", "f");
    has("CallExpr", "f()");
    // WP-2462: the qualifier begins the reference, `&` the address-of.
    has("DeclRefExpr", "B::f");
    has("UnaryOperator", "&B::f");
    has("DeclRefExpr", "s<0>");
    has("DeclRefExpr", "B::s<1>");
    // WP-2463: an array `new`'s list begins at its `(`.
    has("CXXParenListInitExpr", "(1, 2)");
    has("CXXParenListInitExpr", "(1, 2, 3)");
    has("CXXParenListInitExpr", "({R(4)})");
    // WP-2464: `this` begins the access, the `>` ends it.
    assert_eq!(count("MemberExpr", "this->sf<int>"), 2, "{some:?}");
    assert_eq!(count("UnaryOperator", "&this->sf<int>"), 2, "{some:?}");
}

/// Template-ids closed by the first `>` of a `>>`, a `>=` or a `>>=`
/// ([temp.names]/4), and by a `>>` then a `>` (`>>>`).
const SPLITS: &str = r"
template <class T> struct W { static const int v = 1; };
template <class T> int tv = 0;
void f() {
  int a = tv<W<int>>;
  tv<int>= 5;
  tv<W<int>>= 6;
  int c = tv<W<W<int>>>;
}
";

/// The parse of `src`, whatever it diagnoses (kiln accepts a `>=` or a
/// `>>=` closing a template argument list, where clang asks for a space
/// and recovers by the same split).
fn parse_any(src: &str) -> Parsed {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let opts = ParseOptions::default();
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    drop(engine);
    Parsed { sm, interner, tu }
}

/// WP-2542 (spec/ast.md §21.2, spec/lex.md "End of a token"): the AST
/// carries the split `>` locations, so the public token-end query and
/// the client's extents end a template-id at its own `>`.
#[test]
fn a_split_greater_ends_at_its_own_character() {
    let p = parse_any(SPLITS);
    let opts = LexOptions::new(ParseOptions::default().std);
    let splits = &p.tu.ast.split_greaters;
    // Every split location is a `>` that began or continued a longer token.
    let first = *splits.iter().next().expect("split locations");
    let text = p.sm.text(p.sm.offset_of(first).expect("a file location").0);
    for &l in splits {
        let (_, off) = p.sm.offset_of(l).expect("a file location");
        assert_eq!(&text[off as usize..=off as usize], ">");
        let end = kiln_lex::token_end_split(&p.sm, &p.interner, l, opts, splits).unwrap();
        assert_eq!(end, l.with_offset(1));
        // The split-unaware query measures the whole lexed token.
        let whole = kiln_lex::token_end(&p.sm, &p.interner, l, opts).unwrap();
        let (_, we) = p.sm.offset_of(whole).unwrap();
        assert!(we > off + 1 || text.as_bytes()[off as usize - 1] == b'>', "at {off}");
    }
    // `>>` (both halves), `>=`, `>>=` (the `>` and the `>` of the `>=`
    // rest), `>>` + `>` (the `>>` halves).
    let at = |needle: &str, k: usize| -> u32 {
        let base = text.find(needle).expect(needle) + k;
        u32::try_from(base).unwrap()
    };
    let split_offs: std::collections::BTreeSet<u32> =
        splits.iter().map(|&l| p.sm.offset_of(l).unwrap().1).collect();
    for (needle, k) in [
        ("tv<W<int>>;", 8),
        ("tv<W<int>>;", 9),
        ("tv<int>=", 6),
        ("tv<W<int>>=", 8),
        ("tv<W<int>>=", 9),
        ("tv<W<W<int>>>", 10),
        ("tv<W<W<int>>>", 11),
    ] {
        assert!(split_offs.contains(&at(needle, k)), "{needle} +{k}: {split_offs:?}");
    }
    let (some, _) = extents(&p);
    let refs: Vec<&str> =
        some.iter().filter(|(k, _)| k == "DeclRefExpr").map(|(_, t)| t.as_str()).collect();
    for t in ["tv<W<int>>", "tv<int>", "tv<W<W<int>>>"] {
        assert!(refs.contains(&t), "no DeclRefExpr `{t}` among {refs:?}");
    }
    assert!(!refs.iter().any(|t| t.ends_with('=')), "{refs:?}");
}

/// WP-2543 (spec/ast.md §19): a user-defined literal's call and the
/// expression it begins start at the literal token, as clang's
/// `UserDefinedLiteral`; the callee stays at the ud-suffix.
const UDLS: &str = r#"
unsigned long long operator""_k(unsigned long long v) { return v; }
long double operator""_f(long double v) { return v; }
int operator""_s(const char *, unsigned long) { return 1; }
int operator""_c(char) { return 2; }
template <char...> int operator""_t() { return 3; }
int operator""_r(const char *) { return 4; }
auto a = 12_k + 1;
auto b = 1.5_f + 1;
auto c = "ab"_s + 1;
auto d = "a" "b"_s + 1;
auto e = 'x'_c + 1;
auto g = 42_t + 1;
auto h = 7_r + 1;
"#;

#[test]
fn a_user_defined_literal_begins_at_its_token() {
    let p = parse_src(UDLS);
    let ast = &p.tu.ast;
    let lc = |l: kiln_source::Loc| {
        let x = p.sm.line_col(l).expect("a file location");
        (x.line, x.col)
    };
    let mut calls = Vec::new();
    let mut binops = Vec::new();
    for e in written_exprs(ast) {
        match ast.exprs.get(e).kind() {
            kiln_ast::expr::ExprKind::CallExpr => {
                assert!(ast.is_udl_call(e));
                let callee = ast.exprs.callee(e);
                calls.push((lc(ast.begin_loc(e)), lc(ast.end_loc(e)), lc(ast.begin_loc(callee))));
            }
            kiln_ast::expr::ExprKind::BinaryOperator => binops.push(lc(ast.begin_loc(e))),
            _ => {}
        }
    }
    // (begin, end, callee) per line; the callee at the suffix (clang 19
    // and 22 alike), a concatenated string's end at its last piece.
    let want = [
        ((8, 10), (8, 10), (8, 12)),
        ((9, 10), (9, 10), (9, 13)),
        ((10, 10), (10, 10), (10, 14)),
        ((11, 10), (11, 14), (11, 17)),
        ((12, 10), (12, 10), (12, 13)),
        ((13, 10), (13, 10), (13, 12)),
        ((14, 10), (14, 10), (14, 11)),
    ];
    assert_eq!(calls, want);
    assert_eq!(binops, (8..=14).map(|l| (l, 10)).collect::<Vec<_>>());
}

/// WP-2544 (spec/pp.md "`#embed`"): an `#embed`'s bytes are spelled in a
/// scratch buffer behind one `Embed` expansion entry whose expansion is
/// the directive's `#`, so every byte leads back to the directive (clang
/// gives each byte the `#`'s location): the whole run is the `#`, one
/// byte of it has no file range of its own.
#[test]
fn embed_bytes_lead_back_to_their_directive() {
    let src = "int a[] = {\n#embed \"d.bin\"\n};\nint b[] = {\n  #  embed \"d.bin\" limit(1)\n};\n";
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let opts = ParseOptions::default();
    let mut ppo = kiln_pp::PpOptions::new(opts.std);
    ppo.parser_pragmas = true;
    let mut pp = kiln_pp::Preprocessor::new(&sm, &engine, &interner, ppo);
    pp.set_include_resolver(Box::new(
        kiln_pp::MemoryResolver::new().with_resource("d.bin", b"\x01\x02\x03"),
    ));
    pp.enter_main_file(file);
    let target = Target::for_arch(Arch::X86_64);
    let (tu, pp) = kiln_parse::parse_preprocessed(
        pp,
        Arc::clone(&interner),
        &engine,
        target,
        &opts,
        ConstexprLimits::default(),
    );
    drop(pp);
    assert_eq!(engine.error_count(), 0);
    drop(engine);
    let p = Parsed { sm, interner, tu };
    let hashes = [src.find('#').unwrap(), src.rfind('#').unwrap()];
    // One entry per directive, a root, over the `#`.
    let entries: Vec<_> = p.sm.expansion_entries(0).collect();
    let embeds: Vec<_> =
        entries.iter().filter(|e| e.info.kind == kiln_source::ExpansionKind::Embed).collect();
    assert_eq!(embeds.len(), 2);
    for (e, &h) in embeds.iter().zip(&hashes) {
        assert_eq!(e.parent, None);
        let (f, lo) = p.sm.offset_of(e.info.expansion.lo).unwrap();
        let (_, hi) = p.sm.offset_of(e.info.expansion.hi).unwrap();
        assert_eq!((f, lo as usize, hi as usize), (file, h, h + 1));
    }
    // Every byte's literal is in an `Embed` entry: the bytes of the first
    // directive, then the one of the second.
    let ast = &p.tu.ast;
    let lits: Vec<_> = written_exprs(ast)
        .into_iter()
        .filter(|&e| ast.exprs.get(e).kind() == kiln_ast::expr::ExprKind::IntegerLiteral)
        .collect();
    assert_eq!(lits.len(), 4);
    for &e in &lits {
        let x = p.sm.expansion_info(ast.exprs.get(e).loc()).expect("a macro location");
        assert_eq!(x.kind, kiln_source::ExpansionKind::Embed);
    }
    let lexo = LexOptions::new(opts.std);
    let range = |a: kiln_source::Loc, b: kiln_source::Loc| {
        kiln_lex::token_range(&p.sm, &p.interner, a, b, lexo)
            .map(|(f, lo, hi)| (f, lo as usize, hi as usize))
    };
    let loc = |e| ast.exprs.get(e).loc();
    // The whole run of the first embed: its `#`; one byte alone: none.
    assert_eq!(range(loc(lits[0]), loc(lits[2])), Some((file, hashes[0], hashes[0] + 1)));
    assert_eq!(range(loc(lits[1]), loc(lits[1])), None);
    // A one-byte resource is the whole run.
    assert_eq!(range(loc(lits[3]), loc(lits[3])), Some((file, hashes[1], hashes[1] + 1)));
    // The braced lists keep their written text.
    let (some, _) = extents(&p);
    assert!(some.iter().any(|(k, t)| k == "InitListExpr" && t == "{\n#embed \"d.bin\"\n}"));
}

/// WP-2722..WP-2724: a qualified name rebuilt from an overload set of
/// static and non-static members, a single-element braced list argument,
/// and a member access naming a member variable template's
/// specialization.
const LISTS: &str = r"
struct Q {
  static int f(const Q&, int);
  int f(int) const;
  template <class T> static int t(const Q&, T);
  template <class T> int t(T) const;
  int g() const { return Q::f(*this, 1) + Q::f(2) + Q::t<int>(*this, 3) + Q::t<int>(4); }
};
struct O { bool a = false; long b = 0; };
int f(const O&);
int g(O);
int use(O x) { O v{x}; return f({x}) + g({x}) + v.a; }
struct N { N(int); int a; };
int nuse() { N n{2}; return n.a; }
struct V { template <int N> static constexpr int twice = 2 * N; };
int k(int);
int vuse(V d, V* p) { return k(d.twice<7>) + k(p->twice<2>); }
";

/// Every expression's `(kind, client extent text, Ast::begin_loc..end_loc
/// text)` of `src` parsed with the written-reference table `on` or off.
fn both_views(src: &str, on: bool) -> Vec<(String, Option<String>, Option<String>)> {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let opts = ParseOptions { written_refs: on, ..ParseOptions::default() };
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    assert_eq!(engine.error_count(), 0, "the source has errors");
    drop(engine);
    let lexo = LexOptions::new(opts.std);
    let x = ExprExtents::new(&tu.ast, &sm, &interner, lexo);
    let text = |f, b: u32, e: u32| sm.text(f)[b as usize..e as usize].to_owned();
    written_exprs(&tu.ast)
        .into_iter()
        .map(|e| {
            let kind = format!("{:?}", tu.ast.exprs.get(e).kind());
            let client = x.written_range(e).map(|(f, b, end)| text(f, b, end));
            let (b, end) = (tu.ast.begin_loc(e), tu.ast.end_loc(e));
            let ast = kiln_lex::token_range(&sm, &interner, b, end, lexo)
                .map(|(f, lo, hi)| text(f, lo, hi));
            (kind, client, ast)
        })
        .collect()
}

#[test]
fn rebuilt_names_lists_and_member_variable_templates_have_their_text() {
    for on in [false, true] {
        let v = both_views(LISTS, on);
        let has = |kind: &str, text: &str| {
            assert!(
                v.iter().any(|(k, c, a)| k == kind
                    && c.as_deref() == Some(text)
                    && a.as_deref() == Some(text)),
                "written refs {on}: no {kind} `{text}` among {:?}",
                v.iter().filter(|(k, _, _)| k == kind).collect::<Vec<_>>()
            );
        };
        // WP-2722: the qualifier begins the name the call resolves to.
        has("DeclRefExpr", "Q::f");
        has("CallExpr", "Q::f(*this, 1)");
        has("MemberExpr", "Q::f");
        has("CXXMemberCallExpr", "Q::f(2)");
        has("DeclRefExpr", "Q::t<int>");
        has("MemberExpr", "Q::t<int>");
        // WP-2723: a copy from a list's single element is the list's
        // construction, braces included; a direct-list-initialized
        // variable's begins at its name.
        has("CXXConstructExpr", "{x}");
        has("CXXConstructExpr", "v{x}");
        has("CXXConstructExpr", "n{2}");
        // WP-2724: the `>` ends the access.
        has("MemberExpr", "d.twice<7>");
        has("MemberExpr", "p->twice<2>");
    }
}

/// A name after a `decltype(...)::` or splice specifier
/// ([dcl.type.decltype], [expr.prim.id.qual]/1, [temp.names]/3).
const SPECIFIERS: &str = r"
struct S {
  static int m;
  static int f() { return 1; }
  struct N { static int k; };
};
template <class T> struct X { static int v; };
template <class T> int X<T>::v = 0;
S s;
int y = decltype(s)::m;
int z = decltype(s)::N::k;
int (*pf)() = &decltype(s)::f;
int c = decltype(s)::f();
template <class T> int g(T t) { return decltype(t)::m; }
int u = g(s);
int a = [:^^S:]::m;
int b = template [:^^X:]<int>::v;
";

/// WP-2828 (spec/ast.md §21.1): the specifier begins the qualified name,
/// as clang's `getBeginLoc` (19 and 22) begins `decltype(s)::m` at the
/// `decltype`; a splice's begins at its `[:`, or at the `template` of a
/// splice-specialization-specifier.
#[test]
fn a_specifier_begins_the_name_it_qualifies() {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", SPECIFIERS.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let opts = ParseOptions { reflection: true, ..ParseOptions::default() };
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    assert_eq!(engine.error_count(), 0, "the source has errors");
    drop(engine);
    let p = Parsed { sm, interner, tu };
    let (some, _) = extents(&p);
    let has = |kind: &str, text: &str| {
        assert!(
            some.iter().any(|(k, t)| k == kind && t == text),
            "no {kind} `{text}` among {:?}",
            some.iter().filter(|(k, _)| k == kind).collect::<Vec<_>>()
        );
    };
    has("DeclRefExpr", "decltype(s)::m");
    has("DeclRefExpr", "decltype(s)::N::k");
    has("DeclRefExpr", "decltype(s)::f");
    has("UnaryOperator", "&decltype(s)::f");
    has("CallExpr", "decltype(s)::f()");
    has("DependentScopeDeclRefExpr", "decltype(t)::m");
    has("DeclRefExpr", "[:^^S:]::m");
    has("DeclRefExpr", "template [:^^X:]<int>::v");
}

/// WP-3691 (spec/ast.md §21.1): a call to an explicit-object member function
/// written with member syntax (`s.f(2)`, `(*p).f(2)`, `p->f(2)`, `this->f(2)`,
/// instantiated `t.f(2)`) begins at the object argument, while an implicit-
/// object call (`f(2)`) begins at `f`, a non-member call (`(&S::f)(s, 2)`)
/// begins at its callee, and an explicit-object `operator()` (`s(2)`) already
/// begins at `s`.
#[test]
fn explicit_object_member_calls_begin_at_the_object() {
    let src = r"
struct S {
  int f(this S, int);
  int operator()(this S, int);
  int m() { return f(2) + this->f(2); }
};
int g(S s, S *p) {
  return s.f(2) + (*p).f(2) + p->f(2) + (&S::f)(s, 2) + s(2);
}
template <class T> int tmpl(T t) { return t.f(2); }
int h(S s) { return tmpl(s); }
";
    let parsed = parse_src(src);
    let (some, _) = extents(&parsed);
    let has = |kind: &str, text: &str| {
        assert!(
            some.iter().any(|(k, t)| k == kind && t == text),
            "no {kind} `{text}` among {:?}",
            some.iter().filter(|(k, _)| k == kind).collect::<Vec<_>>()
        );
    };
    has("CallExpr", "s.f(2)");
    has("CallExpr", "(*p).f(2)");
    has("CallExpr", "p->f(2)");
    has("CallExpr", "this->f(2)");
    has("CallExpr", "f(2)");
    has("CallExpr", "(&S::f)(s, 2)");
    has("CXXOperatorCallExpr", "s(2)");
    // Also verify the instantiated `t.f(2)` inside `tmpl<S>` (not walked by
    // `written_exprs`, which visits patterns only).
    let expr_extents = ExprExtents::new(
        &parsed.tu.ast,
        &parsed.sm,
        &parsed.interner,
        LexOptions::new(ParseOptions::default().std),
    );
    let inst_calls: Vec<String> = (1..=parsed.tu.ast.exprs.len())
        .map(|idx| kiln_ast::ExprId::new(u32::try_from(idx).unwrap()))
        .filter(|&expr| parsed.tu.ast.exprs.get(expr).kind() == kiln_ast::expr::ExprKind::CallExpr)
        .filter_map(|expr| {
            let (file, begin, end) = expr_extents.written_range(expr)?;
            let text = &parsed.sm.text(file)[begin as usize..end as usize];
            (text == "t.f(2)").then(|| text.to_owned())
        })
        .collect();
    // Pattern `t.f(2)` + instantiated `tmpl<S>` `t.f(2)` = 2 CallExprs.
    assert_eq!(
        inst_calls.len(),
        2,
        "expected both pattern and instantiated t.f(2): {inst_calls:?}"
    );
    // `Ast::begin_loc` agrees with `WrittenExtents::get` on every member-syntax
    // explicit-object call.
    let written = WrittenExtents::new(&parsed.tu.ast);
    let mut member_syntax = 0usize;
    for idx in 1..=parsed.tu.ast.exprs.len() {
        let expr = kiln_ast::ExprId::new(u32::try_from(idx).unwrap());
        let node = parsed.tu.ast.exprs.get(expr);
        if node.kind() == kiln_ast::expr::ExprKind::CallExpr
            && node.has(kiln_ast::expr::ExprFlags::MEMBER_SYNTAX)
        {
            member_syntax += 1;
            let (first, _) = written.get(expr).expect("member-syntax call has a written extent");
            assert_eq!(parsed.tu.ast.begin_loc(expr), first);
        }
    }
    // `f(2)`, `this->f(2)`, `s.f(2)`, `(*p).f(2)`, `p->f(2)`, and instantiated `t.f(2)`.
    assert_eq!(member_syntax, 6);
}

/// WP-3704 (spec/ast.md §21): written extents of instantiated expressions
/// across a function template and a class template member, each instantiated
/// three ways (sharing non-dependent nodes, rebuilding with the same kind,
/// and rebuilding with a different kind).
const INSTANTIATED: &str = r"
namespace n {
  int f(int x) { return x + 1; }
  int v = 10;
  template <class U> int gt(U u) { return int(u); }
  template <class U> constexpr int nv = int(sizeof(U));
  template <class U> concept Small = sizeof(U) <= 16;
}

template <class U> int g(U u) { return int(u); }

int h() { return 0; }
int h(int a) { return a; }
int h(int a, int b) { return a + b; }
int def(int a, int b = 3) { return a + b; }

struct RefWrap { const int &r; };
struct Pair { int a, b; };

struct M {
  int ov(int x) const { return x; }
  int ov(double x) const { return int(x); }
};

struct Ops {
  int v;
  static constexpr int x = 11;
  template <class U> static constexpr int tv = int(sizeof(U));
  Ops() : v(0) {}
  Ops(int a) : v(a) {}
  operator int() const { return v; }
  Ops operator+(const Ops &o) const { return Ops(v + o.v); }
  bool operator==(const Ops &o) const { return v == o.v; }
  template <class U> int f() const { return v + int(sizeof(U)); }
};

struct Alt {
  int v;
  enum { x = 22 };
  template <class U> static constexpr int tv = int(sizeof(U)) + 1;
  Alt() : v(1) {}
  Alt(int a) : v(a) {}
  operator int() const { return v; }
  template <class U> int f(this Alt self) { return self.v + int(sizeof(U)); }
};
Alt operator+(Alt a, Alt b) { return Alt(a.v + b.v); }
bool operator==(Alt a, Alt b) { return a.v == b.v; }

struct StaticMem {
  int v;
  static constexpr int x = 33;
  template <class U> static constexpr int tv = int(sizeof(U)) + 2;
  StaticMem() : v(2) {}
  StaticMem(int a) : v(a) {}
  operator int() const { return v; }
  template <class U> static int f() { return int(sizeof(U)) + 3; }
};

int fn_ptr_target(int x) { return x + 2; }
int alt_fn_target(Alt a) { return a.v + 3; }

struct Functor {
  int operator()(Ops o) const { return o.v + 4; }
};

int h(Ops a, Ops b) { return a.v + b.v; }

template <class T, class Call, int N, class... Ts>
int fn_tmpl(T t, Call c, Ts... xs) {
  M m;
  const int &r_nd = 1 + 2;
  const RefWrap &rw = RefWrap(1);
  const T &r_dep = T{};
  Pair pr = Pair(1, N);
  T tmp = T{};
  tmp.~T();
  bool small = n::Small<T>;
  int shared = n::f(1) + n::v;
  int ov = m.ov(t);
  int arith = int(t + t) + int(t != t);
  int called = c(t);
  int tid = g<T>(t) + n::gt<T>(t) + n::nv<T>;
  int packed = h(xs...) + h(int(xs)...);
  int folded = int((xs + ... + t));
  bool empty_or_all = (bool(xs) && ...);
  int d = def(int(t));
  auto lam = [&](int a) { return a + N + n::f(1) + int(t); };
  return shared + ov + arith + called + tid + packed + folded
         + int(empty_or_all) + int(small) + d + lam(N) + r_nd + rw.r + pr.b + int(r_dep);
}

struct Base {
  int bm = 5;
  int bm_fn() const { return bm; }
  int ov(int x) const { return x; }
  int ov(double x) const { return int(x); }
};

template <class T, int N>
struct Box : Base {
  int member(T t, int k) {
    M m;
    const int &r_nd = 1 + 2;
    const T &r_dep = T{};
    int shared = n::f(1) + Base::bm + Base::bm_fn();
    int qual = T::x + T::template tv<int>;
    int mem_tid = t.template f<int>() + t.operator int();
    int tid = g<T>(t) + n::gt<T>(t);
    int ov = m.ov(t) + Base::ov(t);
    auto lam = [&](int a) { return a + N + k + n::f(1); };
    return shared + qual + mem_tid + tid + ov + lam(N) + r_nd + int(r_dep);
  }
};

int use_all() {
  int a = fn_tmpl<int, int(*)(int), 3>(1, &fn_ptr_target, 10, 20);
  int b = fn_tmpl<Ops, Functor, 5>(Ops(2), Functor{}, Ops(3), Ops(4));
  int c = fn_tmpl<Alt, int(*)(Alt), 7>(Alt(5), &alt_fn_target);
  Box<Ops, 1> b1;
  Box<Alt, 2> b2;
  Box<StaticMem, 3> b3;
  return a + b + c + b1.member(Ops(1), 1) + b2.member(Alt(2), 2) + b3.member(StaticMem(3), 3);
}
";

/// Every expression node reachable from `body` (statements, local variable
/// initializers, expression children, lambda capture initializers and bodies).
fn body_exprs(ast: &kiln_ast::Ast, body: kiln_ast::StmtId) -> Vec<kiln_ast::ExprId> {
    use kiln_ast::decl::DeclKind;
    use kiln_ast::expr::ExprKind;
    use kiln_ast::stmt::StmtKind;
    use kiln_source::fx::FxHashSet;

    enum Item {
        Stmt(kiln_ast::StmtId),
        Expr(kiln_ast::ExprId),
        Decl(kiln_ast::DeclId),
    }

    let mut out = Vec::new();
    let mut seen = FxHashSet::default();
    let mut stack = vec![Item::Stmt(body)];
    let mut kids = Vec::new();
    while let Some(item) = stack.pop() {
        match item {
            Item::Stmt(s) => match ast.stmts.get(s).kind() {
                StmtKind::CompoundStmt => {
                    let stmts: Vec<_> =
                        ast.lists.iter::<kiln_ast::StmtId>(ast.stmts.compound_body(s)).collect();
                    stack.extend(stmts.into_iter().rev().map(Item::Stmt));
                }
                StmtKind::DeclStmt => {
                    let decls: Vec<_> =
                        ast.lists.iter::<kiln_ast::DeclId>(ast.stmts.decl_stmt_decls(s)).collect();
                    stack.extend(decls.into_iter().rev().map(Item::Decl));
                }
                StmtKind::ExprStmt => stack.push(Item::Expr(ast.stmts.expr_stmt_expr(s))),
                StmtKind::ReturnStmt => {
                    if let Some(e) = ast.stmts.return_value(s) {
                        stack.push(Item::Expr(e));
                    }
                }
                StmtKind::IfStmt => {
                    let i = *ast.stmts.if_data(s);
                    stack.extend(i.els.into_iter().map(Item::Stmt));
                    stack.extend(i.then.into_iter().map(Item::Stmt));
                    stack.extend(i.cond.into_iter().map(Item::Expr));
                    stack.extend(i.cond_var.into_iter().map(Item::Decl));
                    stack.extend(i.init.into_iter().map(Item::Stmt));
                }
                other => panic!("unexpected statement kind in test body: {other:?}"),
            },
            Item::Decl(d) => {
                if ast.decls.get(d).kind() == DeclKind::Var
                    && let Some(init) = ast.decls.var(d).and_then(|v| v.init)
                {
                    stack.push(Item::Expr(init));
                }
            }
            Item::Expr(e) => {
                if !seen.insert(e) {
                    continue;
                }
                out.push(e);
                let n = ast.exprs.get(e);
                match n.kind() {
                    ExprKind::LambdaExpr => {
                        let data = ast.exprs.lambdas.get(ast.exprs.lambda_id(e));
                        stack.extend(data.body.into_iter().map(Item::Stmt));
                        if let Some(inits) = data.inits {
                            let list: Vec<_> = ast.lists.iter::<kiln_ast::ExprId>(inits).collect();
                            stack.extend(list.into_iter().rev().map(Item::Expr));
                        }
                    }
                    // Do not descend into a callee parameter's default expression
                    // (which lives on the callee declaration, outside this body).
                    ExprKind::CXXDefaultArgExpr | ExprKind::CXXDefaultInitExpr => {}
                    ExprKind::SubstNonTypeTemplateParmExpr => {
                        stack.extend(kiln_ast::ExprId::try_new(n.b()).map(Item::Expr));
                    }
                    ExprKind::CXXRewrittenBinaryOperator => {
                        stack.extend(kiln_ast::ExprId::try_new(n.a()).map(Item::Expr));
                    }
                    _ => {
                        kids.clear();
                        ast.exprs.children_into(e, &ast.lists, &mut kids);
                        stack.extend(kids.iter().rev().copied().map(Item::Expr));
                    }
                }
            }
        }
    }
    out
}

/// Whether `e` is one of spec/ast.md §21.2's nothing-written or implicit kinds
/// (whose `WrittenExtents::get` is legitimately `None`).
fn is_legitimately_unwritten(
    ast: &kiln_ast::Ast,
    e: kiln_ast::ExprId,
    implicit_captures: &kiln_source::fx::FxHashSet<kiln_ast::ExprId>,
) -> bool {
    use kiln_ast::expr::ExprKind;
    if implicit_captures.contains(&e) {
        return true;
    }
    let n = ast.exprs.get(e);
    match n.kind() {
        ExprKind::CXXDefaultArgExpr
        | ExprKind::CXXDefaultInitExpr
        | ExprKind::ImplicitValueInitExpr
        | ExprKind::NoInitExpr
        | ExprKind::ArrayInitIndexExpr
        | ExprKind::ArrayInitLoopExpr
        | ExprKind::OpaqueValueExpr => true,
        ExprKind::CXXThisExpr => ast.exprs.this_is_implicit(e),
        ExprKind::CXXConstructExpr => {
            ast.exprs.end_loc(e).is_none() && ast.exprs.children(e, &ast.lists).len() != 1
        }
        // Compiler-synthesized callee of an overloaded operator or implicit
        // conversion function call (when no written name end was recorded).
        ExprKind::DeclRefExpr | ExprKind::MemberExpr => {
            ast.exprs.end_loc(e).is_none()
                && matches!(
                    ast.decls.get(ast.exprs.decl(e)).name().kind(),
                    kiln_ast::names::DeclNameKind::Operator
                        | kiln_ast::names::DeclNameKind::Conversion
                )
        }
        ExprKind::ImplicitCastExpr
        | ExprKind::MaterializeTemporaryExpr
        | ExprKind::CXXBindTemporaryExpr
        | ExprKind::ExprWithCleanups
        | ExprKind::ConstantExpr
        | ExprKind::CXXStdInitializerListExpr => ast
            .exprs
            .first_child(e, &ast.lists)
            .is_some_and(|c| is_legitimately_unwritten(ast, c, implicit_captures)),
        _ => false,
    }
}

/// Every expression inside an implicit lambda capture initializer in `ast`.
fn implicit_capture_inits(ast: &kiln_ast::Ast) -> kiln_source::fx::FxHashSet<kiln_ast::ExprId> {
    use kiln_ast::ids::LambdaId;
    use kiln_ast::lambda::CaptureKind;
    let mut out = kiln_source::fx::FxHashSet::default();
    let mut stack = Vec::new();
    for i in 1..=ast.exprs.lambdas.len() {
        let l = ast.exprs.lambdas.get(LambdaId::new(u32::try_from(i).unwrap()));
        let Some(inits) = l.inits else { continue };
        let inits = ast.lists.get(inits);
        if inits.len() != l.captures.len() {
            continue;
        }
        for (c, &init) in l.captures.iter().zip(inits) {
            if c.implicit || c.kind == CaptureKind::VlaBound {
                stack.push(kiln_ast::ExprId::new(init));
            }
        }
    }
    let mut kids = Vec::new();
    while let Some(e) = stack.pop() {
        if out.insert(e) {
            kids.clear();
            ast.exprs.children_into(e, &ast.lists, &mut kids);
            stack.extend_from_slice(&kids);
        }
    }
    out
}

type SpecNodeList = Vec<(kiln_ast::ExprId, kiln_ast::expr::ExprKind, Option<String>)>;

fn assert_has_node(nodes: &SpecNodeList, kind: kiln_ast::expr::ExprKind, text: &str) {
    assert!(
        nodes.iter().any(|(_, k, t)| *k == kind && t.as_deref() == Some(text)),
        "missing {kind:?} `{text}` in {:?}",
        nodes.iter().filter(|(_, k, _)| *k == kind).map(|(_, _, t)| t).collect::<Vec<_>>()
    );
}

#[allow(clippy::too_many_arguments)]
fn check_group_all_nodes(
    p: &Parsed,
    tree: &WrittenExtents<'_>,
    client: &ExprExtents<'_>,
    implicit_captures: &kiln_source::fx::FxHashSet<kiln_ast::ExprId>,
    label: &str,
    pat_fn: kiln_ast::DeclId,
    specs: &[kiln_ast::DeclId],
    extra_synthesized: &[(&str, &str)],
) {
    use kiln_ast::expr::ExprKind;
    use kiln_source::fx::{FxHashMap, FxHashSet};

    let ast = &p.tu.ast;
    let text_of = |e: kiln_ast::ExprId| -> Option<String> {
        let (f, b, end) = client.written_range(e)?;
        Some(p.sm.text(f)[b as usize..end as usize].to_owned())
    };
    let pat_body = ast.decls.function(pat_fn).and_then(|f| f.body).expect("pattern body");
    let pat_nodes = body_exprs(ast, pat_body);
    let pat_ids: FxHashSet<_> = pat_nodes.iter().copied().collect();
    let mut pat_extents: FxHashMap<(kiln_source::Loc, kiln_source::Loc), Vec<ExprKind>> =
        FxHashMap::default();
    for &pe in &pat_nodes {
        if let Some(ext) = tree.get(pe) {
            pat_extents.entry(ext).or_default().push(ast.exprs.get(pe).kind());
        }
    }
    let mut problems = Vec::new();
    let (mut any_shared, mut any_rebuilt_same, mut any_rebuilt_diff) = (false, false, false);
    for (idx, &sp) in specs.iter().enumerate() {
        let body = ast.decls.function(sp).and_then(|f| f.body).expect("specialization body");
        for e in body_exprs(ast, body) {
            let kind = ast.exprs.get(e).kind();
            let ext = tree.get(e);
            let txt = text_of(e);
            assert_eq!(ext, client.written_extent(e), "{label}[{idx}] {kind:?}");
            if pat_ids.contains(&e) {
                any_shared = true;
                continue;
            }
            match ext {
                Some(ext) => {
                    if let Some(pat_kinds) = pat_extents.get(&ext) {
                        if pat_kinds.contains(&kind) {
                            any_rebuilt_same = true;
                        } else {
                            any_rebuilt_diff = true;
                        }
                    } else {
                        let kstr = format!("{kind:?}");
                        let allowed = txt.as_deref().is_some_and(|t| {
                            extra_synthesized.iter().any(|&(ek, et)| ek == kstr && et == t)
                        });
                        if !allowed {
                            problems.push(format!(
                                "{label}[{idx}] {kind:?} rebuilt extent {txt:?} not in pattern"
                            ));
                        }
                    }
                }
                None if !is_legitimately_unwritten(ast, e, implicit_captures) => {
                    let lc = p.sm.line_col(ast.exprs.get(e).loc());
                    problems.push(format!(
                        "{label}[{idx}] {kind:?} at {lc:?} has None extent (not legitimately unwritten)"
                    ));
                }
                None => {}
            }
        }
    }
    assert!(any_shared, "{label}: expected shared nodes");
    assert!(any_rebuilt_same, "{label}: expected rebuilt same-kind nodes");
    assert!(any_rebuilt_diff, "{label}: expected rebuilt different-kind nodes");
    assert!(problems.is_empty(), "{label} extent discrepancies:\n{}", problems.join("\n"));
}

fn check_fn_tmpl_constructs(
    p: &Parsed,
    client: &ExprExtents<'_>,
    fn_pat: kiln_ast::DeclId,
    fn_specs: &[kiln_ast::DeclId],
) {
    use kiln_ast::expr::ExprKind;
    use kiln_source::fx::FxHashSet;

    let ast = &p.tu.ast;
    let text_of = |e: kiln_ast::ExprId| -> Option<String> {
        let (f, b, end) = client.written_range(e)?;
        Some(p.sm.text(f)[b as usize..end as usize].to_owned())
    };
    let spec_nodes = |fn_decl: kiln_ast::DeclId| -> SpecNodeList {
        let body = ast.decls.function(fn_decl).and_then(|f| f.body).expect("body");
        body_exprs(ast, body)
            .into_iter()
            .map(|e| (e, ast.exprs.get(e).kind(), text_of(e)))
            .collect()
    };
    let fn_pat_ids: FxHashSet<_> =
        body_exprs(ast, ast.decls.function(fn_pat).unwrap().body.unwrap()).into_iter().collect();
    let [fn0, fn1, fn2] =
        [spec_nodes(fn_specs[0]), spec_nodes(fn_specs[1]), spec_nodes(fn_specs[2])];

    for nodes in [&fn0, &fn1, &fn2] {
        let nf_calls: Vec<_> = nodes
            .iter()
            .filter(|(_, k, t)| *k == ExprKind::CallExpr && t.as_deref() == Some("n::f(1)"))
            .collect();
        assert!(nf_calls.len() >= 2, "expected n::f(1) in body and lambda");
        assert!(
            nf_calls.iter().all(|(e, _, _)| fn_pat_ids.contains(e)),
            "n::f(1) should be shared"
        );
        let mov: Vec<_> = nodes
            .iter()
            .filter(|(_, k, t)| *k == ExprKind::MemberExpr && t.as_deref() == Some("m.ov"))
            .collect();
        assert_eq!(mov.len(), 1);
        assert!(!fn_pat_ids.contains(&mov[0].0), "m.ov must be rebuilt, not shared");
        let mat_nd: Vec<_> = nodes
            .iter()
            .filter(|(_, k, t)| {
                *k == ExprKind::MaterializeTemporaryExpr && t.as_deref() == Some("1 + 2")
            })
            .collect();
        assert_eq!(mat_nd.len(), 1);
        assert!(!fn_pat_ids.contains(&mat_nd[0].0), "extended temporary must be unshared");
        for (k, text) in [
            (ExprKind::CXXParenListInitExpr, "RefWrap(1)"),
            (ExprKind::CXXParenListInitExpr, "(1, N)"),
            (ExprKind::CXXFunctionalCastExpr, "Pair(1, N)"),
            (ExprKind::MaterializeTemporaryExpr, "T{}"),
            (ExprKind::DeclRefExpr, "g<T>"),
            (ExprKind::CallExpr, "g<T>(t)"),
            (ExprKind::DeclRefExpr, "n::gt<T>"),
            (ExprKind::CallExpr, "n::gt<T>(t)"),
            (ExprKind::DeclRefExpr, "n::nv<T>"),
            (ExprKind::ConceptSpecializationExpr, "n::Small<T>"),
            (ExprKind::SubstNonTypeTemplateParmExpr, "N"),
            (ExprKind::LambdaExpr, "[&](int a) { return a + N + n::f(1) + int(t); }"),
            (ExprKind::CallExpr, "h(xs...)"),
            (ExprKind::CallExpr, "h(int(xs)...)"),
            (ExprKind::ParenExpr, "(xs + ... + t)"),
        ] {
            assert_has_node(nodes, k, text);
        }
        assert!(nodes.iter().any(|(_, k, t)| *k == ExprKind::CXXDefaultArgExpr && t.is_none()));
    }

    for (nodes, kind, text) in [
        (&fn0, ExprKind::BinaryOperator, "t + t"),
        (&fn0, ExprKind::BinaryOperator, "t != t"),
        (&fn0, ExprKind::CallExpr, "c(t)"),
        (&fn0, ExprKind::CXXPseudoDestructorExpr, "tmp.~T"),
        (&fn0, ExprKind::ParenExpr, "(bool(xs) && ...)"),
        (&fn0, ExprKind::BinaryOperator, "bool(xs)"),
        (&fn1, ExprKind::CXXOperatorCallExpr, "t + t"),
        (&fn1, ExprKind::CXXRewrittenBinaryOperator, "t != t"),
        (&fn1, ExprKind::CXXOperatorCallExpr, "c(t)"),
        (&fn1, ExprKind::MemberExpr, "tmp.~T"),
        (&fn1, ExprKind::ParenExpr, "(bool(xs) && ...)"),
        (&fn1, ExprKind::BinaryOperator, "bool(xs)"),
        (&fn2, ExprKind::CXXOperatorCallExpr, "t + t"),
        (&fn2, ExprKind::CXXRewrittenBinaryOperator, "t != t"),
        (&fn2, ExprKind::CallExpr, "c(t)"),
        (&fn2, ExprKind::MemberExpr, "tmp.~T"),
        (&fn2, ExprKind::CXXBoolLiteralExpr, "..."),
    ] {
        assert_has_node(nodes, kind, text);
    }

    for (nodes, expected_args) in [(&fn0, 2usize), (&fn1, 2usize), (&fn2, 0usize)] {
        for (call_text, elem_text) in [("h(xs...)", "xs"), ("h(int(xs)...)", "int(xs)")] {
            let call = nodes
                .iter()
                .find(|(_, k, t)| *k == ExprKind::CallExpr && t.as_deref() == Some(call_text))
                .unwrap()
                .0;
            let args: Vec<_> =
                ast.lists.iter::<kiln_ast::ExprId>(ast.exprs.call_args(call)).collect();
            assert_eq!(args.len(), expected_args);
            for a in args {
                assert_eq!(text_of(a).as_deref(), Some(elem_text));
            }
        }
    }
}

fn check_box_member_constructs(
    p: &Parsed,
    client: &ExprExtents<'_>,
    box_specs: &[kiln_ast::DeclId],
) {
    use kiln_ast::expr::ExprKind;
    let ast = &p.tu.ast;
    let spec_nodes = |fn_decl: kiln_ast::DeclId| -> SpecNodeList {
        let body = ast.decls.function(fn_decl).and_then(|f| f.body).expect("body");
        body_exprs(ast, body)
            .into_iter()
            .map(|e| {
                let txt = client
                    .written_range(e)
                    .map(|(f, b, end)| p.sm.text(f)[b as usize..end as usize].to_owned());
                (e, ast.exprs.get(e).kind(), txt)
            })
            .collect()
    };
    let [box0, box1, box2] =
        [spec_nodes(box_specs[0]), spec_nodes(box_specs[1]), spec_nodes(box_specs[2])];
    for nodes in [&box0, &box1, &box2] {
        for (k, text) in [
            (ExprKind::DeclRefExpr, "T::x"),
            (ExprKind::DeclRefExpr, "T::template tv<int>"),
            (ExprKind::MemberExpr, "Base::bm"),
            (ExprKind::MemberExpr, "Base::bm_fn"),
            (ExprKind::CXXMemberCallExpr, "Base::bm_fn()"),
            (ExprKind::MemberExpr, "Base::ov"),
            (ExprKind::CXXMemberCallExpr, "Base::ov(t)"),
            (ExprKind::MemberExpr, "t.operator int"),
            (ExprKind::CXXMemberCallExpr, "t.operator int()"),
            (ExprKind::DeclRefExpr, "g<T>"),
            (ExprKind::DeclRefExpr, "n::gt<T>"),
            (ExprKind::SubstNonTypeTemplateParmExpr, "N"),
        ] {
            assert_has_node(nodes, k, text);
        }
    }
    assert_has_node(&box0, ExprKind::CXXMemberCallExpr, "t.template f<int>()");
    assert_has_node(&box0, ExprKind::MemberExpr, "t.template f<int>");
    assert_has_node(&box1, ExprKind::CallExpr, "t.template f<int>()");
    assert_has_node(&box2, ExprKind::CallExpr, "t.template f<int>()");
    assert_has_node(&box2, ExprKind::MemberExpr, "t.template f<int>");
}

#[test]
fn instantiated_expressions_keep_pattern_written_extents() {
    use kiln_ast::decl::DeclKind;

    let p = parse_src(INSTANTIATED);
    let ast = &p.tu.ast;
    let tree = WrittenExtents::new(ast);
    let client =
        ExprExtents::new(ast, &p.sm, &p.interner, LexOptions::new(ParseOptions::default().std));
    let implicit_captures = implicit_capture_inits(ast);
    let find_decl = |kind: DeclKind, name: &str| -> kiln_ast::DeclId {
        (1..=ast.decls.len())
            .map(|i| kiln_ast::DeclId::new(u32::try_from(i).unwrap()))
            .find(|&d| {
                ast.decls.get(d).kind() == kind
                    && ast
                        .decls
                        .get(d)
                        .name()
                        .as_ident()
                        .is_some_and(|id| p.interner.as_str(id) == name)
            })
            .unwrap_or_else(|| panic!("missing {kind:?} `{name}`"))
    };

    let fn_tmpl = find_decl(DeclKind::FunctionTemplate, "fn_tmpl");
    let fn_pat = ast.template_info(fn_tmpl).unwrap().pattern.unwrap();
    let fn_specs: Vec<_> = ast.templates.specs_of(fn_tmpl).to_vec();
    assert_eq!(fn_specs.len(), 3, "expected 3 specializations of fn_tmpl");

    let box_tmpl = find_decl(DeclKind::ClassTemplate, "Box");
    let box_pat = ast.template_info(box_tmpl).unwrap().pattern.unwrap();
    let member_of = |rec: kiln_ast::DeclId| -> kiln_ast::DeclId {
        ast.members(rec)
            .find(|&m| {
                ast.decls.get(m).kind() == DeclKind::CXXMethod
                    && ast
                        .decls
                        .get(m)
                        .name()
                        .as_ident()
                        .is_some_and(|id| p.interner.as_str(id) == "member")
            })
            .expect("Box::member")
    };
    let box_pat_member = member_of(box_pat);
    let box_specs: Vec<_> =
        ast.templates.specs_of(box_tmpl).iter().map(|&c| member_of(c)).collect();
    assert_eq!(box_specs.len(), 3, "expected 3 specializations of Box::member");

    check_group_all_nodes(
        &p,
        &tree,
        &client,
        &implicit_captures,
        "fn_tmpl",
        fn_pat,
        &fn_specs,
        &[
            ("BinaryOperator", "xs + ... + t"),
            ("CXXOperatorCallExpr", "xs + ... + t"),
            ("MaterializeTemporaryExpr", "xs + ... + t"),
            ("ImplicitCastExpr", "xs + ... + t"),
            ("UnaryOperator", "!= t"),
            ("CXXBoolLiteralExpr", "..."),
        ],
    );
    check_group_all_nodes(
        &p,
        &tree,
        &client,
        &implicit_captures,
        "Box::member",
        box_pat_member,
        &box_specs,
        &[("DeclRefExpr", "f")],
    );
    check_fn_tmpl_constructs(&p, &client, fn_pat, &fn_specs);
    check_box_member_constructs(&p, &client, &box_specs);
}
