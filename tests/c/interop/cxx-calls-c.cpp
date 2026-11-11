// [dcl.link]/1-4: C++ calls functions defined in a C translation unit
// through `extern "C"` declarations; the C unit (Inputs/c-impl.c) is
// compiled as C (it uses C++ keywords as identifiers).
//@ expect: run
//@ unit: Inputs/c-impl.c
//@ stdout: 10 3
#include <cstdio>
extern "C" {
int c_sum(const int *v, int n);
extern int c_calls;
}
int main() {
  const int v[] = {1, 2, 3, 4};
  int s = c_sum(v, 2) + c_sum(v + 2, 2);
  std::printf("%d %d\n", s, c_calls + 1);
}
