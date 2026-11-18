// [temp.inst]/4: a friend function defined in a class template is
// instantiated when the function is odr-used, also when the use (through
// a namespace-scope redeclaration) comes before the class template
// specialization that defines it is instantiated. Found running GCC's C++
// testsuite, where kiln failed to link (WP-1645).
//@ expect: run
//@ stdout: 14 14
#include <cstdio>

template <class V> struct Box {
  friend int twice_plus(const V &v) { return 2 * v + 6; }
};
int twice_plus(const int &);

int before() { return twice_plus(4); }
template <class W> int later(const W &) { return twice_plus(4); }

int main() {
  const Box<int> b;
  std::printf("%d %d\n", before(), later(b));
}
