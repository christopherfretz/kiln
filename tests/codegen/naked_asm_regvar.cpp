// [dcl.fct.def.general], [dcl.asm] (GNU extensions): register variables
// with an asm label inside a `naked` function body. The body has no
// parameter prologue (the incoming argument is read from its register by
// the asm, not spilled), the register variable is still an `alloca` that
// the asm operand loads and stores pinned to `{reg}`, and the body ends
// in `unreachable` (`llvm.trap` first at -O0). The register names are
// per target: AArch64 `x0`/`w1`, x86-64 `rdi`/`esi`.
#if defined(__aarch64__)
__attribute__((naked)) long nr(long a) {
  register long x asm("x0");
  asm volatile("add %0, %0, #1\n\tret" : "+r"(x));
}
__attribute__((naked)) int nr2(int a, int b) {
  register int y asm("w1");
  register long z asm("x0");
  asm volatile("add %w1, %w1, %w0\n\tret" : "=r"(z) : "r"(y));
}
#else
__attribute__((naked)) long nr(long a) {
  register long x asm("rdi");
  asm volatile("lea 1(%0), %%rax\n\tret" : "+r"(x));
}
__attribute__((naked)) int nr2(int a, int b) {
  register int y asm("esi");
  register long z asm("rdi");
  asm volatile("lea (%0,%1), %%eax\n\tret" : "=r"(z) : "r"(y));
}
#endif
long use(long v) { return nr(v) + nr2(1, 2); }
