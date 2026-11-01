//@ expect: run
//@ mem<=256
// The test binary gets the bare budget: a 1 GiB allocation dies with
// std::bad_alloc.
#include <cstdio>
int main() {
  volatile char* p = new char[1u << 30];
  p[0] = 1;
  std::printf("%d\n", p[0]);
}
