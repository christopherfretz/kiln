// [class.mem.general]/7, [dcl.fct.default]/5, [class.local]: a default
// argument of a member of a (nested) local class is a complete-class
// context of the outermost local class, so it names an enumerator
// declared later. In a function template kiln substituted it at the
// member's declaration and dropped it silently ("too few arguments" at
// the call; WP-905). g++ 14 accepts and prints the same.
//@ expect: run
//@ skip(clang): clang 19 and 22 reject it ("no member '' in 'O'; it has not yet been instantiated"), a clang bug; the non-template twin compiles
//@ stdout: 4 6 7
#include <cstdio>

template <class T> int f() {
  struct O {
    struct I { int g(int x = LATE2) { return x; } };
    int h(int y = LATE2 + 2, T z = sizeof(T) + LATE2 - 1) { return y + 0 * z; }
    T k(T z = LATE2 + LATE3) { return z; }
    enum { LATE2 = 4, LATE3 = 3 };
  };
  typename O::I i;
  O o;
  std::printf("%d %d %d\n", i.g(), o.h(), o.k());
  return i.g() + o.h() + o.k() == 17 ? 0 : 1;
}

int main() { return f<int>(); }
