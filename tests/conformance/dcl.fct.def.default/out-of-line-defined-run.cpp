//@ expect: run
//@ strict-diags
//@ std: c++20
//@ stdout: 9 9 3
// [dcl.fct.def.default]/5: an explicitly-defaulted function that is not
// defaulted on its first declaration is user-provided and implicitly
// defined at the point of its `= default` (clang `SetDeclDefaulted` →
// `DefineDefaultedFunction`); a member of a class template whose pattern
// is defaulted out of line is defined when the member is instantiated.
// Both must be emitted, or the program does not link.
#include <cstdio>
int dtors = 0;
struct M { int m = 7; ~M() { ++dtors; } };
struct Y { int x = 2; M m; Y(); Y(const Y &); ~Y(); };
Y::Y() = default;
Y::Y(const Y &) = default;
Y::~Y() = default;
template <class T> struct Z { T x = 2; M m; Z(); Z(const Z &); ~Z(); };
template <class T> Z<T>::Z() = default;
template <class T> Z<T>::Z(const Z &) = default;
template <class T> Z<T>::~Z() = default;
int main() {
  int a, b;
  {
    Y y;
    Y z(y);
    a = y.x + z.m.m;
  }
  {
    Z<int> y;
    Z<int> z(y);
    b = y.x + z.m.m;
  }
  std::printf("%d %d %d\n", a, b, dtors - 1);
}
