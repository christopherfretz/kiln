// [basic.start.main]: the value returned from main is the exit status.
//@ expect: run
//@ stdout: before return
//@ exit: 3
//@ xfail(kiln): libstdc++ headers do not parse yet: kiln_parse does not support decltype (c++config.h)
#include <cstdio>

int main() {
  std::puts("before return");
  return 3;
}
