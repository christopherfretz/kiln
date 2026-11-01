// [stmt.switch]: a value-dependent case label defers every check of the
// case list, -Wswitch-default included, to the instantiation (clang
// `HasDependentValue`; WP-230)
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wswitch-default -Wswitch-enum
//@ diag: 17:3 warning "'switch' missing 'default' label"
//@ diag: 23:29 note "in instantiation of function template specialization 'h<3>' requested here"
//@ diag: 18:1 warning "non-void function does not return a value in all control paths"
//@ diag: 20:3 warning "'switch' missing 'default' label"
//@ diag: 23:39 note "in instantiation of function template specialization 'k<1>' requested here"
//@ diag: 20:11 warning "enumeration value 'C' not handled in switch"
enum E { A, B, C };
template <int N> int f(int x) { switch (x) { case N: return 1; default: return 0; } }
template <int N> int g(int x) { switch (x) { default: return 0; case N: return 1; } }
template <int N> int h(int x) {
  switch (x) { case 1: return 2; case N: return 1; }
}
template <int N> int k(E x) {
  switch (x) { case A: return 2; case (E)N: return 1; }
  return 0;
}
int u = f<3>(1) + g<3>(1) + h<3>(1) + k<1>(A);
