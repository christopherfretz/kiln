//@ expect: ast
//@ oracle: clang
// [class.copy.elision]/3: the implicit-move xvalue cast wraps the whole
// return or throw operand, including a resolved `__builtin_choose_expr` or
// GNU `__extension__` (both looked through by clang `IgnoreParens`): an
// `ImplicitCastExpr <NoOp>` xvalue around the operand, which stays an
// lvalue (WP-1176). `_Generic` takes the same path
// (implicit-move-through-generic-selection-run.cpp); the dependent case is
// implicit-move-through-choose-expr-dependent-ast.cpp.
struct M {
  M(M &&) = default;
  M(const M &) = delete;
};
// Parameters: implicitly movable, and no NRVO candidates (clang's JSON
// dump omits `nrvo_candidate`; NRVO is pinned by
// nrvo-through-generic-selection-ir.cpp).
M choose(M m) { return (__builtin_choose_expr(1, m, 0)); }
M extension(M m) { return __extension__ m; }
void thrown(M &&r) {
  M m = static_cast<M &&>(r);
  throw __builtin_choose_expr(0, 0, m);
}
