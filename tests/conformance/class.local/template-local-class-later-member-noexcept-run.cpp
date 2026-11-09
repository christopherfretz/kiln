// [class.mem.general]/7, [except.spec]/2, [class.local]: the
// noexcept-specifier of a member of a local class (and of its member
// class) names an enumerator declared later in the outermost local class.
// kiln left it potentially-throwing in a function template (WP-905).
// g++ 14 accepts and prints the same.
//@ expect: run
//@ skip(clang): clang 19 and 22 reject it ("no member '' in 'O'; it has not yet been instantiated"), a clang bug; the non-template twin compiles
//@ stdout: 1 1
#include <cstdio>

template <class T> int f() {
  struct O {
    int g() noexcept(NE) { return 1; }
    struct I { int g() noexcept(NE) { return 1; } };
    enum : bool { NE = true };
  };
  O o;
  typename O::I i;
  std::printf("%d %d\n", noexcept(o.g()), noexcept(i.g()));
  return noexcept(o.g()) + noexcept(i.g()) == 2 ? 0 : 1;
}

int main() { return f<int>(); }
