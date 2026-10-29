// [basic.start.main]: the value returned from main is the exit status.
//@ expect: run
//@ stdout: before return
//@ exit: 3
//@ xfail(kiln): libstdc++ headers do not parse yet: function overloading (c++config.h), __builtin_va_list and asm labels (stdio.h)
#include <cstdio>

int main() {
  std::puts("before return");
  return 3;
}
