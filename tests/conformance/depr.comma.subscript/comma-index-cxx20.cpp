// [depr.comma.subscript], C++20 [expr.sub]/1: before C++23 the subscript is
// one expression, so a top-level comma is the comma operator (deprecated in
// C++20) and a class's operator[] gets one argument: a multi-parameter
// operator[] declared only under __cpp_multidimensional_subscript is not
// needed. A parenthesized comma is not deprecated.
//@ std: c++20
//@ expect: run
//@ strict-diags
//@ diag: 24:19 warning "top-level comma expression in array subscript is deprecated in C++20 and unsupported in C++23"
//@ diag: 25:14 warning "top-level comma expression in array subscript is deprecated in C++20 and unsupported in C++23"
//@ diag: 24:15 warning "left operand of comma operator has no effect"
//@ diag: 24:18 warning "left operand of comma operator has no effect"
//@ diag: 24:31 warning "left operand of comma operator has no effect"
//@ diag: 25:13 warning "left operand of comma operator has no effect"
//@ diag: 26:14 warning "left operand of comma operator has no effect"
struct S {
  int operator[](int i) { return i; }
#if defined(__cpp_multidimensional_subscript)
  int operator[](int, int, int) { return -1; }
#endif
};
int a[4] = {1, 2, 3, 4};
int main() {
  int r = S{}[0, 1, 2] + S{}[(0, 1)];
  int k = a[0, 3];
  int j = a[(0, 2)];
  return r + k + j - 10;
}
