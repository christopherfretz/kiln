// [expr.const]/10 (P2280R4, a defect report): unknown references in C++11.
// An array reference parameter passed on, a member call that reads
// nothing, and the address of the unknown object compared with itself
// (clang 22 accepts that from C++23; GCC 16.1 rejects the comparison).
// Found running GCC's C++ testsuite.
//@ std: c++11
//@ xfail(clang): clang 22 applies P2280 from C++23 only
typedef decltype(sizeof 0) size_t;
template <class T, size_t N> constexpr size_t size(T (&)[N]) { return N; }
struct S { constexpr int three() const { return 3; } };
void f(bool (&param)[2], const S &s, int &x) {
  static_assert(size(param) == 2, "");
  static_assert(s.three() == 3, "");
  static_assert(&x == &x, "");
  static_assert(&param[0] != &param[1], "");
}
