// [class.copy.elision]/3, [expr.prim.id.unqual]/4: the operand of a return
// statement or throw-expression that is a (possibly parenthesized)
// id-expression naming an implicitly movable entity is an xvalue. clang
// strips the operand with `Expr::IgnoreParens`, which also looks through
// GNU `__extension__` and a resolved `_Generic` selection or
// `__builtin_choose_expr` (`getNamedReturnInfo`, `IsThrownVarInScope`), so
// a move-only local returned or thrown through one is moved, not copied
// (WP-1176). A result-dependent selection in a template is looked through
// once instantiation resolves it.
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 3 4 5 6 7
//@ stdout: thrown 8
#include <cstdio>

struct M {
  int v;
  explicit M(int x) : v(x) {}
  M(M &&o) : v(o.v) {}
  M(const M &) = delete;
};

M choose() {
  M m(1);
  return __builtin_choose_expr(1, m, 0);
}
M choose_second() {
  M m(2);
  return __builtin_choose_expr(0, 0, (m));
}
M generic() {
  M m(3);
  return _Generic(0, int: m, default: 0);
}
M generic_default() {
  M m(4);
  return _Generic(0, default: (m));
}
M nested() {
  M m(5);
  return (_Generic(0, int: (__builtin_choose_expr(1, (m), 0))));
}
M extension() {
  M m(6);
  return __extension__ m;
}
template <class T> T generic_dependent(T t) { return _Generic(t, M: t, default: t); }

int main() {
  std::printf("%d %d %d %d %d %d %d\n", choose().v, choose_second().v, generic().v,
              generic_default().v, nested().v, extension().v, generic_dependent(M(7)).v);
  try {
    M m(8);
    throw _Generic(0, int: m);
  } catch (M &e) {
    std::printf("thrown %d\n", e.v);
  }
}
