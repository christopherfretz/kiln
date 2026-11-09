// [class.mem.general]/6: an enumeration may be introduced in a class with an
// opaque-enum-declaration and later redeclared there with an enum-specifier.
// In a class template the instantiation declares the enumeration at the first
// member and defines it at the second, once (clang 22 and GCC accept; clang 19
// reports "enumerator 'x' does not exist in instantiation"). Found by WP-600.
//@ expect: run
//@ strict-diags
//@ requires: clang>=22
//@ stdout: 1 2 3
#include <cstdio>
template <class T>
struct A {
  enum E : int;
  enum E : int { x = 1, y };
  enum class F : int;
  enum class F : int { z = 3 };
  int g() const { return x; }
  static E e() { return y; }
};
int main() { std::printf("%d %d %d\n", A<int>().g(), (int)A<int>::e(), (int)A<int>::F::z); }
