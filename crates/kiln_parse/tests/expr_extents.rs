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
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", SRC.to_owned()).expect("location space");
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
