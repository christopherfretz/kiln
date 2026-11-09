// [class.mem.general]/7, [class.local], [temp.inst]/3: the default member
// initializer of a member class of a local class is a complete-class
// context of the outermost local class too, so it names members declared
// after it. In a function template kiln substituted it at the nested
// class's end and silently dropped the reference (the fields were 0, a
// miscompile; WP-905). Now every complete-class context part of a local
// class waits for the outermost class, as the parser replays them.
//@ expect: run
//@ stdout: 3 5 4 7 7 8
#include <cstdio>

template <class T> int f() {
  struct O {
    struct I {
      int a = LATE;
      int b = O::sf();
      struct J { T c = LATE + 4; } j;
    };
    int c = LATE + 1;
    enum { LATE = 3 };
    static int sf() { return 5; }
    struct K { int d = sf() + 2; int e = O::sf() + 2; int g = LATE + 5; };
  };
  O o;
  typename O::I i;
  typename O::K k;
  std::printf("%d %d %d %d %d %d\n", i.a, i.b, o.c, i.j.c, k.d, k.g);
  return i.a + i.b + o.c + i.j.c + k.d + k.e + k.g == 3 + 5 + 4 + 7 + 7 + 7 + 8 ? 0 : 1;
}

int main() { return f<int>(); }
