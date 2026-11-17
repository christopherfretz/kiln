// [expr.const]/10 (P2280R4, a defect report against C++11 on): a reference
// not usable in constant expressions binds to an object of unknown value,
// in every language mode (as GCC 16.1; clang 22 applies it from C++23
// only, `p2280-unknown-reference.cpp`). Naming the reference, passing it
// on, a member call that reads nothing and `sizeof` are constant in C++20.
// Found running GCC's C++ testsuite.
//@ std: c++20
//@ oracle: gcc
//@ xfail(clang): clang 22 applies P2280 from C++23 only
using size_t = decltype(sizeof 0);
template <class T, size_t N> constexpr size_t size(T (&)[N]) { return N; }
void array_param(int (&a)[4]) {
  static_assert(size(a) == 4);
  static_assert(sizeof(a) == 4 * sizeof(int));
}
struct S { constexpr int three() const { return 3; } };
void class_param(const S &s) { static_assert(s.three() == 3); }
extern S &es;
static_assert(es.three() == 3);
template <int N> struct Array { constexpr int count() const { return N; } };
template <int> void tmpl() {}
void template_argument(const Array<7> &a) { tmpl<a.count()>(); }
int main() {
  S s;
  // A by-reference capture names the enclosing variable.
  [&] { static_assert(s.three() == 3); }();
}
