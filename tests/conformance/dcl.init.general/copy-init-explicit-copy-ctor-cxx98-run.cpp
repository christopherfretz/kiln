// C++98 [dcl.init]/14: copy-initialization through a converting
// constructor makes a temporary and then direct-initializes the object
// from it, so an explicit copy constructor serves that copy, for a
// variable, a parameter and a returned object alike (clang and GCC
// agree). Found running GCC's C++ testsuite.
//@ expect: run
//@ std: c++98
//@ stdout: 9
#include <cstdio>

struct X {
  int v;
  X(int i) : v(i) {}
  explicit X(X const &o) : v(o.v) {}
};

X make() { return 4; }
int take(X x) { return x.v; }

int main() {
  X x = 2;
  std::printf("%d\n", x.v + take(3) + make().v);
}
