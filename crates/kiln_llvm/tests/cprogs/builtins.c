// Overflow builtins, expect, trap, unreachable.
int mulchk(unsigned long a, unsigned long b, unsigned long *r) {
  return __builtin_mul_overflow(a, b, r);
}
void die(int x) {
  if (__builtin_expect(x, 0))
    __builtin_trap();
}
int never(void) { __builtin_unreachable(); }
