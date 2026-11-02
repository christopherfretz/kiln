//@ expect: ast
//@ oracle: clang
// [expr.call]/5, [temp.dep.expr]/1: in a template, a call of a
// pseudo-destructor through a dependent object is a `void` CallExpr
// (parenthesized, a `void` ParenExpr) whose uses stay dependent: `&` of
// it is formed at instantiation (a dependent UnaryOperator), and
// `decltype` of it is a dependent type.
template <class T> void f(T *p, T t) {
  p->~T();
  (t.~T());
  using D = decltype(p->~T());
  (void)&t.~T();
}
