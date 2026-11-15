// [expr.prim.this]/2-3, [class.local]: a member function of a local class
// declared in a lambda in a default member initializer has its own
// `this`; an implicit member access in it names the local class's member,
// not the enclosing class's. Found running GCC's C++ testsuite
// (g++.dg/contracts).
//@ expect: run
//@ std: c++20
//@ stdout: 3 4
#include <cstdio>
struct U {
  int x = [] {
    struct In {
      int i = 1;
      int bump() { return i += 2; }
    };
    In in;
    return in.bump();
  }();
  int y = [] {
    struct In {
      int i = 4;
      int get() const { return i; }
    } in;
    return in.get();
  }();
};
int main() {
  U u;
  std::printf("%d %d\n", u.x, u.y);
}
