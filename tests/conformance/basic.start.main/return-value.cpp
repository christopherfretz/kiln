// [basic.start.main]: the value returned from main is the exit status.
//@ expect: run
//@ stdout: before return
//@ exit: 3
#include <cstdio>

int main() {
  std::puts("before return");
  return 3;
}
