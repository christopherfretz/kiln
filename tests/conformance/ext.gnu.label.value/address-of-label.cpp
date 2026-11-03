// GNU labels as values (an extension; [stmt.label], [stmt.goto]): `&&label`
// is a prvalue of type `void *` naming a label of the enclosing function,
// declared by the use if not yet seen, with "use of GNU address-of-label
// extension" under -Wgnu-label-as-value. Outside a function body it is an
// error at the label name; a non-identifier operand is "expected
// identifier"; a label never defined is diagnosed at the function's end.
// Returning one is -Wreturn-stack-address, once in a template definition
// and again per instantiation. Code generation and constant evaluation
// are covered by stmt.goto/computed-goto-*.cpp.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wgnu-label-as-value
//@ diag: 27:14 error "use of address-of-label extension outside of a function body"
//@ diag: 30:13 warning "use of GNU address-of-label extension"
//@ diag: 31:19 warning "use of GNU address-of-label extension"
//@ diag: 31:24 warning "use of GNU address-of-label extension"
//@ diag: 32:17 warning "use of GNU address-of-label extension"
//@ diag: 33:13 warning "use of GNU address-of-label extension"
//@ diag: 34:5 error "expected identifier"
//@ diag: 33:15 error "use of undeclared label 'nowhere'"
//@ diag: 39:10 warning "use of GNU address-of-label extension"
//@ diag: 39:10 warning "returning address of label, which is local"
//@ diag: 41:41 warning "use of GNU address-of-label extension"
//@ diag: 41:41 warning "returning address of label, which is local"
//@ diag: 41:41 warning "returning address of label, which is local"
//@ diag: 42:11 note "in instantiation of function template specialization 'h<int>' requested here"
void *gp = &&top;
void f(int x) {
  a:;
  void *p = &&a;
  void *tab[] = { &&a, &&later };
  bool b = x && &&a != p;
  void *s = &&nowhere;
  &&5;
later:;
}
void *g() {
  r:
  return &&r;
}
template<class T> void *h() { x: return &&x; }
void *u = h<int>();
