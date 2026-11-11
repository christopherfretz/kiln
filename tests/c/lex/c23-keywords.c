// C23 6.4.1: the C23 keywords bool, true, false, nullptr, constexpr,
// static_assert, alignas, alignof, thread_local and typeof; u8 character
// constants (6.4.4.5) and digit separators (6.4.4.1); `true` is 1 in #if.
//@ std: c23
//@ expect: run
//@ stdout: 1 1101 1
#include <stdio.h>
#if !true
#error
#endif
static_assert(sizeof(int) >= 2);
alignas(16) static int buf[4];
thread_local int tl;
int main(void) {
  bool b = true;
  constexpr int k = 1'000;
  typeof(k) z = k + (int)u8'a' + (int)alignof(int);
  int *q = nullptr;
  printf("%d %d %d\n", (int)b, z, q == nullptr);
  return buf[0];
}
