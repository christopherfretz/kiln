// GNU computed goto and labels as values ([stmt.goto], an extension): the
// `-Wgnu-label-as-value` warnings are pedantic extension diagnostics, at
// the `*` of `goto *` and at the `&&` of `&&label`. The operand of `goto *`
// is still checked (here: an unexpanded pack, [temp.variadic]/6).
//@ expect: compile-fail
//@ strict-diags
//@ args: -pedantic
//@ diag: 12:8 warning "use of GNU indirect-goto extension"
//@ diag: 12:9 error "expression contains unexpanded parameter pack 'xs'"
//@ diag: 16:13 warning "use of GNU address-of-label extension"
template<class... T> void f(T... xs) {
  goto *xs;
}
void g() {
  l:;
  void *p = &&l;
  (void)p;
}
