// [expr.eq]/2, [expr.rel]/2 (C++26, P2865R6): equality and relational
// comparison of two operands of array type is ill-formed; comparing an
// array with a pointer (array-to-pointer conversion of one operand) or
// after an explicit decay is still valid. As in clang, operands whose
// array type is dependent, or reached through a reference, are not
// diagnosed in a template's instantiation.
//@ expect: compile-fail
//@ std: c++26
//@ strict-diags
//@ requires: clang>=22
//@ diag: 21:22 error "comparison between two arrays is ill-formed in C++26; to compare array addresses, use unary '+' to decay operands to pointers [-Warray-compare-cxx26]"
//@ diag: 21:22 warning "array comparison always evaluates to false [-Wtautological-compare]"
//@ diag: 22:22 error "comparison between two arrays is ill-formed in C++26; to compare array addresses, use unary '+' to decay operands to pointers [-Warray-compare-cxx26]"
//@ diag: 22:22 warning "array comparison always evaluates to true [-Wtautological-compare]"
//@ diag: 23:22 error "comparison between two arrays is ill-formed in C++26; to compare array addresses, use unary '+' to decay operands to pointers [-Warray-compare-cxx26]"
//@ diag: 23:22 warning "array comparison always evaluates to a constant [-Wtautological-compare]"
//@ diag: 24:22 error "comparison between two arrays is ill-formed in C++26; to compare array addresses, use unary '+' to decay operands to pointers [-Warray-compare-cxx26]"
//@ diag: 24:22 warning "array comparison always evaluates to a constant [-Wtautological-compare]"
int a[2], b[2];
int* p = a;
bool eq() { return a == b; }
bool ne() { return a != b; }
bool lt() { return a < b; }
bool ge() { return a >= b; }
bool with_ptr() { return a == p && p != b; }
bool decayed() { return +a == +b; }
template <class T> bool cmp(T& x, T& y) { return x == y; }
bool use() { return cmp(a, b); }
template <int N> bool sized() {
  static int x[N], y[N];
  return x == y;
}
bool use_sized() { return sized<3>(); }
