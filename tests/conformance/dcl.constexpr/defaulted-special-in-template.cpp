// [dcl.constexpr]/3, [expr.const]/22: a constexpr defaulted special
// member used in a template definition is defined there, as the
// definition's constant initializers are evaluated without an
// instantiation (clang `NeededForConstantEvaluation`). The constinit
// static member of a class template follows the folly `FastStaticBool`
// pattern; the explicitly defaulted constructor has a default member
// initializer, so it is not trivial.
//@ expect: run
//@ stdout: 7 7 7 9
#include <cstdio>

struct at {
  int v = 7;
  at() noexcept = default;
  constexpr at(int x) : v(x) {}
  constexpr at(const at&) = default;
};

template <class I> int local() {
  constexpr at a{};
  return a.v;
}

template <class I> struct F {
  static constinit at flag;
};
template <class I> constinit at F<I>::flag{};

template <class I> constexpr at vt{};

template <class I> int copied() {
  constexpr at src(9);
  constexpr at a = src;
  return a.v;
}

int main() {
  std::printf("%d %d %d %d\n", local<int>(), F<int>::flag.v, vt<int>.v, copied<int>());
}
