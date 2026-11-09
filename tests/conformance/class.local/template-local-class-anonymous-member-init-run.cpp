// [class.union.anon], [class.mem.general]/7, [class.local]: the default
// member initializers of anonymous union and struct members of a local
// class in a function template name the function's enumerator and static
// local. kiln instantiated them lazily without the function's locals and
// reported "not yet implemented" (WP-905). (g++ 14 ICEs on the `s` case.)
//@ expect: run
//@ stdout: 3 4 3
#include <cstdio>

template <class T> int f() {
  enum E { A = 2 };
  static int s = 3;
  struct O {
    union { int u = A + 1; float fl; };
    struct { int x = A + 2; } an;
    struct { int y = s; };
  };
  O o;
  std::printf("%d %d %d\n", o.u, o.an.x, o.y);
  return o.u + o.an.x + o.y == 10 ? 0 : 1;
}

int main() { return f<int>(); }
