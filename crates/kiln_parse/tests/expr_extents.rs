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
