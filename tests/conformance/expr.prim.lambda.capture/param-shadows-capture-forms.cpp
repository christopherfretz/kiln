// [expr.prim.lambda.capture]/5: a lambda parameter does not have the name
// of an explicit capture: a by-reference or by-copy simple-capture, a
// capture pack, in a generic lambda and in a lambda with a template head;
// each one is reported at the parameter, with a note at the capture. Valid
// pair: param-shadows-capture-valid.cpp (WP-1015).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 22:21 error "a lambda parameter cannot shadow an explicitly captured entity"
//@ diag: 22:11 note "variable 'y' is explicitly captured here"
//@ diag: 22:28 error "a lambda parameter cannot shadow an explicitly captured entity"
//@ diag: 22:14 note "variable 'z' is explicitly captured here"
//@ diag: 23:24 error "a lambda parameter cannot shadow an explicitly captured entity"
//@ diag: 23:10 note "variable 'x' is explicitly captured here"
//@ diag: 24:18 error "a lambda parameter cannot shadow an explicitly captured entity"
//@ diag: 24:10 note "variable 'x' is explicitly captured here"
//@ diag: 27:21 error "a lambda parameter cannot shadow an explicitly captured entity"
//@ diag: 27:10 note "variable 'ts' is explicitly captured here"
//@ diag: 30:15 error "a lambda parameter cannot shadow an explicitly captured entity"
//@ diag: 30:10 note "variable 't' is explicitly captured here"
void f() {
  int x = 0, y = 0, z = 0;
  (void)[&y, z](int y, int z) { return y + z; };
  (void)[x]<class T>(T x) { return 0; };
  (void)[x](auto x) { return 0; };
}
template<class... Ts> void g(Ts... ts) {
  (void)[ts...](int ts) { return ts; };
}
template<class T> void h(T t) {
  (void)[t](T t) { return 0; };
}
void use() { g(1, 2); h(1); }
