// [dcl.asm] (GNU extension): a condition-flag output `=@cc<cond>` reads
// one condition after the template, and an operand naming a `register`
// variable with an asm label uses exactly that register.
//@ expect: run
//@ stdout: 1 0 0 1 42 43
#include <stdio.h>
#if defined(__aarch64__)
bool eq(long a, long b) {
  bool z;
  asm("cmp %1, %2" : "=@cceq"(z) : "r"(a), "r"(b));
  return z;
}
bool lt(long a, long b) {
  bool l;
  asm("cmp %1, %2" : "=@cclt"(l) : "r"(a), "r"(b));
  return l;
}
long pinned(long a) {
  register long in asm("x9") = a;
  register long out asm("x10");
  // The template names the registers directly, not through operands.
  asm volatile("add x10, x9, #1" : "=r"(out) : "r"(in));
  return out;
}
#else
bool eq(long a, long b) {
  bool z;
  asm("cmp %2, %1" : "=@ccz"(z) : "r"(a), "r"(b));
  return z;
}
bool lt(long a, long b) {
  bool l;
  asm("cmp %2, %1" : "=@ccl"(l) : "r"(a), "r"(b));
  return l;
}
long pinned(long a) {
  register long in asm("r9") = a;
  register long out asm("r10");
  asm volatile("lea 1(%%r9), %%r10" : "=r"(out) : "r"(in));
  return out;
}
#endif
int main() {
  printf("%d %d %d %d %ld %ld\n", eq(3, 3), eq(3, 4), lt(5, 2), lt(-1, 2), pinned(41), pinned(42));
}
