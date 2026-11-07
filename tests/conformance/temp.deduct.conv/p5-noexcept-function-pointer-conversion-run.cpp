// [temp.deduct.conv]/5: when the original A is a pointer to function
// or a pointer to member function, the deduced A may be a pointer to
// `noexcept` function, reached through a function pointer conversion
// ([conv.fctptr]). A generic lambda's `noexcept` conversion function
// template and a conversion template to `int (*)(T) noexcept` both
// convert to a plain function pointer; kiln rejected them.
//@ std: c++17
//@ expect: run
//@ strict-diags
//@ stdout: 2 3 6 7
#include <cstdio>
template <class T> using fp = int (*)(T) noexcept;
struct X { int m(int) noexcept { return 7; } };
template <class T> using mfp = int (X::*)(T) noexcept;
int twice(int v) noexcept { return 2 * v; }
struct C {
  template <class T> operator fp<T>() const { return twice; }
  template <class T> operator mfp<T>() const { return &X::m; }
};
int main() {
  auto g = [](auto v) noexcept { return v + 1; };
  int (*a)(int) = g;
  int (*b)(int) noexcept = g;
  int (*c)(int) = C();
  int (X::*d)(int) = C();
  X x;
  std::printf("%d %d %d %d\n", a(1), b(2), c(3), (x.*d)(4));
}
