// [stmt.goto] (GNU computed goto), [temp.variadic]/6: the operand of
// `goto *` is an expression, so an unexpanded pack in it is ill-formed.
// The pack check runs on the operand before the conversion to `void *`
// (spec/sema.md, indirect goto).
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
