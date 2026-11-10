//@ expect: ast
//@ ast-filter: dependent
//@ skip(clang): clang-ast cannot normalize a function template with instantiations yet (spec/ast-dump.md §5); the golden was checked against the clang 19 and 22 text dumps
// [class.copy.elision]/3, [temp.dep.expr]: a condition-dependent
// `__builtin_choose_expr` in a template is not looked through (clang
// `IgnoreParens` stops at `isConditionDependent`), so the return operand
// gets no implicit-move cast in the template; the instantiation resolves
// the choice and wraps it in the `ImplicitCastExpr <NoOp>` xvalue, and
// the move constructor is selected (WP-1176).
struct M {
  M() = default;
  M(M &&) = default;
  M(const M &) = delete;
};
template <class T> T dependent(T t) { return __builtin_choose_expr(__is_class(T), t, 0); }
M use() { return dependent(M()); }
