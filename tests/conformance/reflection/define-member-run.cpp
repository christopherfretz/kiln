//@ expect: run
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ stdout: 1 42 c 24
//@ stdout: 7 2.5
// [meta.reflection.define.aggregate] P2996R13 (spike vehicle
// `__kiln_define_member`): codegen lays out and emits the injected member
// like a declared one; the program reads and writes it at run time.
#include "kiln_meta.h"
extern "C" int printf(const char*, ...);
using namespace std::meta;

struct S {
  int a;
  static_assert(__kiln_define_member(^^S, ^^long, "injected") != info{});
  char c;
  long get() const { return injected; }
};

template <class T> struct G {
  int tag;
  static_assert(__kiln_define_member(^^G, ^^T, "v") != info{});
};

int main(int argc, char**) {
  S s{argc, 7, 'c'};
  s.injected *= 6;
  printf("%d %ld %c %zu\n", s.a, s.get(), s.c, sizeof(S));
  G<double> g{7, 0.5};
  g.v += argc * 2;
  printf("%d %.1f\n", g.tag, g.v);
  return 0;
}
