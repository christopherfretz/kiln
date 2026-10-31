//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:8 error "non-const reference cannot bind to bit-field 'a'"
//@ diag: 7:17 note "bit-field is declared here"
// [dcl.init.ref]/5: a binding to a bit-field is a bit-field lvalue, so a
// non-const lvalue reference cannot bind to it (a const one binds a copy).
struct BF { int a : 3; int b; };
void f(BF bf) {
  auto &[x, y] = bf;
  int &r = x;
  const int &cr = x;
  int &s = y;
}
