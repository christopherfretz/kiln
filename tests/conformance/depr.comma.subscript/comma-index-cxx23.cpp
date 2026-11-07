// C++23 [expr.sub]/1: the subscript is an expression-list, so `x[0, 1, 2]`
// calls a three-parameter operator[] and a top-level comma is no longer the
// comma operator; a parenthesized comma still is, without deprecation.
//@ std: c++23
//@ expect: run
//@ strict-diags
//@ diag: 13:31 warning "left operand of comma operator has no effect"
struct S {
  int operator[](int i) { return i; }
  int operator[](int a, int b, int c) { return a + b + c + 10; }
};
int main() {
  int r = S{}[0, 1, 2] + S{}[(0, 1)];
  return r - 14;
}
