// Dependent operator expressions (spec/ast.md §18.13.6): an operator whose
// operand is type-dependent ([temp.dep.expr]/1) is bound at instantiation
// ([temp.dep.candidate]/1). Each operator token is a DEPENDENT entry with
// the found set of its definition context: the non-member candidates of
// unqualified lookup ([over.match.oper]/3.2) and the ADL flag
// ([basic.lookup.argdep]); a using-directive that made a candidate
// visible is recorded ([namespace.udir]/2). Each specialization that
// reaches the token has a row: the operator function selected (`how=adl`,
// `how=member`), a rewritten or reversed candidate ([over.match.oper]/3.4,
// /8) with REWRITTEN and REVERSED, `<=>` then `<` for `a < b` through a
// user `operator<=>` whose result has a user `operator<`
// ([over.match.oper]/8), or `(builtin)` for a built-in operator
// ([over.built]). A template never instantiated and the discarded branch
// of `if constexpr` ([stmt.if]/2) have the entry and no row. A generic
// lambda's call operator template keys its rows by its own
// specialization ([expr.prim.lambda.closure]/5).
//@ written-refs: operators-dependent.refs
namespace lib {
struct S {
  int v;
  S &operator+=(int);
  S &operator++();
  S operator++(int);
};
S operator+(S, S);
S operator-(S);
bool operator==(S, S);

struct R {
  friend bool operator<(R, int);
};
struct A {
  R operator<=>(const A &) const;
};
struct B {
  int v;
};
bool operator==(B, int);
} // namespace lib

namespace ops {
lib::B operator*(lib::B, lib::B);
} // namespace ops
using namespace ops;

template <class T> T add(T a, T b) { return a + b; }
template <class T> T neg(T a) { return -a; }
template <class T> void bump(T &a) {
  a += 1;
  ++a;
  a++;
  a = a;
}
template <class T> bool ne(T a, T b) { return a != b; }
template <class T> bool lt(T a, T b) { return a < b; }
template <class T> bool rev(T a) { return 1 == a; }
template <class T> T mul(T a) { return a * a; }
template <class T> T never(T a) { return a + a; }
template <class T> int discard(T a) {
  if constexpr (sizeof(T) > 1000)
    return a + a;
  else
    return 0;
}
template <class T> auto gen(T t) {
  return [t](auto u) { return t + u; };
}

int run() {
  lib::S s{};
  add(s, s);
  add(1, 2);
  neg(s);
  neg(1.0);
  bump(s);
  int i = 0;
  bump(i);
  ne(s, s);
  ne(1, 2);
  lib::A a;
  lt(a, a);
  rev(lib::B{});
  mul(lib::B{});
  mul(3);
  gen(s)(s);
  gen(1)(2);
  return discard(1);
}
