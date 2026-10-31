//@ expect: ast
//@ oracle: clang
// [over.match.oper]/3.4 (spec/sema-overload.md §6.3): rewritten `!=`,
// reversed `==` and rewritten relational operators build
// CXXRewrittenBinaryOperator around their semantic form.
struct A { bool operator==(int) const; };
struct O { int operator<=>(const O &) const; };
bool f(A a, O o) {
  return (a != 1) && (1 == a) && (1 != a) && (o < o) && (o >= o);
}
