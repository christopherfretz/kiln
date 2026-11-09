// [except.spec]/2, [class.mem.general]/7, [class.local]: the
// noexcept-specifier of a local class member in a function template names
// the function's local enumerator and constant, the class's own
// enumerator, and the template parameter. kiln instantiated it on demand
// without the function's locals, so `noexcept(FNE)` and `noexcept(NE)`
// silently became potentially-throwing (WP-905).
//@ expect: run
//@ stdout: 1 1 0 1 1 1
#include <cstdio>

template <class T> int f() {
  enum : bool { FNE = true };
  constexpr bool c = true;
  struct O {
    enum : bool { NE = true };
    int g() noexcept(FNE) { return 1; }
    int h() noexcept(NE) { return 1; }
    int i() noexcept(sizeof(T) == 1) { return 1; }
    int j() noexcept(c) { return 1; }
    struct I {
      int k(int x) noexcept(NE && noexcept(x + FNE)) { return x; }
      static int m() noexcept(FNE) { return 0; }
    };
  };
  O o;
  typename O::I i;
  std::printf("%d %d %d %d %d %d\n", noexcept(o.g()), noexcept(o.h()), noexcept(o.i()),
              noexcept(o.j()), noexcept(i.k(1)), noexcept(O::I::m()));
  return noexcept(o.g()) && noexcept(o.h()) && !noexcept(o.i()) ? 0 : 1;
}

int main() { return f<int>(); }
