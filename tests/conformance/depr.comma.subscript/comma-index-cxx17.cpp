// C++17 [expr.sub]/1: the subscript is one expression; a top-level comma is
// the comma operator and is not yet deprecated.
//@ std: c++17
//@ expect: run
//@ strict-diags
//@ diag: 13:15 warning "left operand of comma operator has no effect"
//@ diag: 13:25 warning "left operand of comma operator has no effect"
struct S {
  int operator[](int i) { return i; }
};
int a[4] = {1, 2, 3, 4};
int main() {
  int r = S{}[0, 1] + a[1, 2];
  return r - 4;
}
