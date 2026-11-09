// [dcl.fct.def.default]/2, [class.copy.assign]/1, /12: a defaulted copy or
// move assignment operator with an explicit object parameter is a special
// member; its implicit definition assigns each subobject of the object
// parameter (named or not) and returns it. WP-958.
//@ std: c++23
//@ requires: clang>=22
//@ expect: run
//@ stdout: 7 3 9
#include <cstdio>
struct B {
  int b = 0;
};
struct S : B {
  S &operator=(this S &, const S &) = default;
  int v = 1;
};
struct M {
  M &operator=(this M &self, M &&) = default;
  int v = 2;
};
int main() {
  S a, c;
  c.v = 7;
  c.b = 3;
  S &r = (a = c);
  M m, n;
  n.v = 9;
  m = static_cast<M &&>(n);
  std::printf("%d %d %d\n", r.v, a.b, m.v);
}
