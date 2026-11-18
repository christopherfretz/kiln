// GNU `__restrict` on a member function: `this` is `X *__restrict`, so
// at -O2 a store through a plain pointer parameter cannot change a member
// read back through `this` and the call to the never-defined `unreachable_wp`
// folds away (clang 22 marks `this` `noalias`; GCC 16.1 folds it too).
// The program itself has no aliasing: it is well-defined either way.
// WP-1663.
//@ expect: run
//@ args: -O2
//@ stdout: 7 1
#include <cstdio>
void unreachable_wp();

struct Box {
  int v;
  void fill(int *out) __restrict {
    v = 7;
    *out = 1;
    if (v != 7)
      unreachable_wp();
  }
};

int main() {
  Box b{0};
  int o = 0;
  b.fill(&o);
  std::printf("%d %d\n", b.v, o);
}
