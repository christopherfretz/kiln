//@ expect: run
//@ stdout: 1
// 1 GiB of address space fits the default cap.
#include <cstdio>
int main() {
  volatile char* p = new char[1u << 30];
  p[0] = 1;
  std::printf("%d\n", p[0]);
}
