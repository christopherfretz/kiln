// [class.copy.elision]/3: the operand of a return statement that is a
// (possibly parenthesized) id-expression naming an implicitly movable
// entity is treated as an rvalue. clang looks through a resolved
// `_Generic` selection and `__builtin_choose_expr` as through parentheses
// (`Expr::IgnoreParens`), so a move-only local returned through one is
// moved, not copied (WP-1131 repro).
//@ expect: compile-ok
//@ xfail(kiln): WP-1131 follow-up, the implicit-move candidate check does not look through a resolved GenericSelectionExpr/ChooseExpr (call to deleted copy constructor)
struct M {
  M() = default;
  M(M &&) = default;
  M(const M &) = delete;
};
M choose() {
  M m;
  return __builtin_choose_expr(1, m, 0);
}
M generic() {
  M m;
  return _Generic(0, int: m, default: 0);
}
M generic_default() {
  M m;
  return _Generic(0, default: (m));
}
template <class T> T generic_dependent(T t) { return _Generic(t, M: t, default: t); }
M use() { return generic_dependent(M()); }
