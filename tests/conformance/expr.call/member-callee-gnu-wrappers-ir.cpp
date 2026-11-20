// [expr.call]/2 (WP-1739): the callee of a member function call may be
// a member access or a pointer-to-member expression under GNU
// `__extension__`, `__builtin_choose_expr` or `_Generic` as well as
// parentheses (clang `IgnoreParens`); codegen calls the chosen member
// as clang does, and constant evaluation agrees.
//@ expect: ir
struct S {
  int v;
  constexpr int f(int x) const { return v + x; }
  constexpr int g(int x) const { return v * x; }
};

constexpr int k() {
  S s{3};
  const S *p = &s;
  int (S::*m)(int) const = &S::g;
  return (__extension__ s.f)(1) + __builtin_choose_expr(0, s.f, p->g)(3) +
         (__extension__(s.*m))(4) + _Generic(0, int: (p->*m), long: s.f)(5);
}
static_assert(k() == 4 + 9 + 12 + 15);

int run(S &s, S *p, int (S::*m)(int) const) {
  return (__extension__ s.f)(1) + __builtin_choose_expr(1, s.f, s.g)(2) +
         __builtin_choose_expr(0, s.f, p->g)(3) + (__extension__(s.*m))(4) +
         __builtin_choose_expr(1, (p->*m), s.f)(5) + _Generic(1L, int: s.f, long: p->g)(6);
}
