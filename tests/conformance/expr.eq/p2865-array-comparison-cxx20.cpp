// [depr.array.comp] (C++20, P1120R0; removed by P2865R6 in C++26): before
// C++26, equality and relational comparison of two arrays is deprecated
// (clang -Wdeprecated-array-compare); an array compared with a pointer
// is not. As in clang, operands whose array type is dependent, or reached
// through a reference, are not diagnosed in a template's instantiation.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
//@ diag: 15:22 warning "comparison between two arrays is deprecated; to compare array addresses, use unary '+' to decay operands to pointers [-Wdeprecated-array-compare]"
//@ diag: 15:22 warning "array comparison always evaluates to false [-Wtautological-compare]"
//@ diag: 16:22 warning "comparison between two arrays is deprecated; to compare array addresses, use unary '+' to decay operands to pointers [-Wdeprecated-array-compare]"
//@ diag: 16:22 warning "array comparison always evaluates to a constant [-Wtautological-compare]"
int a[2], b[2];
int* p = a;
bool eq() { return a == b; }
bool lt() { return a < b; }
bool with_ptr() { return a == p; }
template <class T> bool cmp(T& x, T& y) { return x == y; }
bool use() { return cmp(a, b); }
template <int N> bool sized() {
  static int x[N], y[N];
  return x == y;
}
bool use_sized() { return sized<3>(); }
