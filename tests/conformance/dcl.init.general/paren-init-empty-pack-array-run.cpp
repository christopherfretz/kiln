// [temp.variadic]/7, [dcl.init.general]/16.4: an array initialized from
// a parenthesized pack expansion that expands to nothing is initialized
// by `()`, value-initialized ([dcl.init.general]/9.2). Found running GCC's
// C++ testsuite.
//@ expect: run
//@ std: c++11
//@ stdout: 0
#include <cstdio>

void dirty() {
  volatile int junk[8];
  for (int i = 0; i < 8; ++i)
    junk[i] = 0x5a5a + i;
}

template <class... T> int sum(T... t) {
  int a[4](t...);
  return a[0] + a[1] + a[2] + a[3];
}

int main() {
  dirty();
  std::printf("%d\n", sum());
}
