// __builtin_choose_expr (GNU) in a template: a value-dependent condition
// leaves the ChooseExpr `<dependent type>` with the condition unevaluated;
// each instantiation evaluates it (a `ConstantExpr`) and takes the chosen
// operand's type and value category (WP-1131, [temp.dep.constexpr]).
//@ expect: ast
//@ oracle: clang
template <class T, int N> struct C {
  static constexpr long v = __builtin_choose_expr(N > 1, T{1}, 2.5);
  static constexpr int w = __builtin_choose_expr(N, 10, 20);
  static constexpr int k = __builtin_choose_expr(1, 3, T{});
};
long a = C<long, 2>::v + C<int, 0>::w + C<int, 0>::k;
