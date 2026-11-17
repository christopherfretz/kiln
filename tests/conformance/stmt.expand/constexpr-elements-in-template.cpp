// [stmt.expand]/5.1, /5.2 (P1306R5): `constexpr` and plain elements of
// enumerating and iterating expansion statements in a function template,
// a generic lambda, a class template's member, nested, and over an empty
// range, are constants in every instantiation. The destructuring form:
// constexpr-destructuring-in-template.cpp. Found running GCC's C++
// testsuite.
//@ args: --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement expansion statements
//@ oracle: gcc
//@ expect: compile-ok
#include <array>
template <int N> constexpr int en() {
  int s = 0;
  template for (constexpr auto i : {N, N + 1, N + 2}) { constexpr int k = i; s += k; }
  template for (auto i : {N, N}) { s += i; }
  return s;
}
static_assert(en<1>() == 8);
constexpr std::array<int, 3> arr{4, 5, 6};
template <class T> constexpr T it() {
  T s = 0;
  template for (constexpr T i : arr) { constexpr T k = i; s += k; }
  template for (constexpr T i : std::array<T, 0>{}) { s += i; }
  return s;
}
static_assert(it<int>() == 15);
constexpr auto gl = [](auto x) {
  decltype(x) s = 0;
  template for (constexpr auto i : arr) { s += i; }
  return s;
};
static_assert(gl(1L) == 15);
template <class T> struct C {
  static constexpr int f() {
    int s = 0;
    template for (constexpr auto i : {1, 2}) {
      template for (constexpr auto j : {10, 20}) { constexpr int k = i * j; s += k; }
    }
    return s;
  }
};
static_assert(C<int>::f() == 90);
