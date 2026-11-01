// [stmt.switch]: -Wswitch-enum/-Wcovered-switch-default/-Wswitch-default inside
// templates and their instantiations (WP-219)
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wswitch-enum -Wcovered-switch-default -Wswitch-default
//@ diag: 28:3 warning "'switch' missing 'default' label"
//@ diag: 28:11 warning "enumeration values 'B' and 'C' not handled in switch"
//@ diag: 30:3 warning "'switch' missing 'default' label"
//@ diag: 30:11 warning "no case matching constant switch condition '2'"
//@ diag: 28:3 warning "'switch' missing 'default' label"
//@ diag: 33:14 note "in instantiation of function template specialization 'f<E>' requested here"
//@ diag: 28:11 warning "enumeration values 'B' and 'C' not handled in switch"
//@ diag: 29:3 warning "'switch' missing 'default' label"
//@ diag: 29:11 warning "enumeration values 'B' and 'C' not handled in switch"
//@ diag: 30:3 warning "'switch' missing 'default' label"
//@ diag: 30:11 warning "no case matching constant switch condition '2'"
//@ diag: 34:14 error "explicit instantiation of 'f' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 27:24 note "candidate template ignored: could not match 'int (E, E)' against 'int (E, int)'"
//@ diag: 36:3 warning "'switch' missing 'default' label"
//@ diag: 39:3 warning "'switch' missing 'default' label"
//@ diag: 39:3 warning "'switch' missing 'default' label"
//@ diag: 41:14 note "in instantiation of function template specialization 'h<int>' requested here"
//@ diag: 42:17 warning "switch condition has boolean value"
//@ diag: 42:17 warning "'switch' missing 'default' label"
//@ diag: 44:70 warning "default label in switch which covers all enumeration values"
enum E { A, B, C };
template <class T> int f(E e, T t) {
  switch (e) { case A: return 1; }
  switch (t) { case A: return 1; }
  switch (2) { case 1: break; }
  return 0;
}
template int f<E>(E, E);
template int f<E>(E, int);
int g(E e) {
  switch (e) { case A: return 1; case B: return 2; case C: return 3; }
}
template <class T> int h(E e) {
  switch (e) { case A: return 1; case B: return 2; case C: return 3; }
}
template int h<int>(E);
int k(bool b) { switch (b) { case true: return 1; case false: return 0; } }
int m(E e) {
  switch (e) { case A: return 1; case B: return 2; case C: return 3; default: break; }
}
