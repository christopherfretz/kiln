// [stmt.goto] (GNU computed goto), [temp.variadic]/6: the operand of
// `goto *` is an expression, so an unexpanded pack in it is ill-formed.
// The computed goto itself is not supported (spec/parse.md §6.1); its
// operand is checked first.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:9 error "expression contains unexpanded parameter pack 'xs'"
//@ diag: 13:9 error "expression contains unexpanded parameter pack 'N'"
template<class... T> void f(T... xs) {
  goto *xs;
}
template<int... N> void g(void **p) {
  goto *p[N];
}
