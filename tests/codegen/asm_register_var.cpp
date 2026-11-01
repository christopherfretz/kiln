// [dcl.asm] (GNU extension): an operand naming a `register` variable with
// an asm label is pinned to that register: `{reg}` (normalized, `&{reg}`
// when early-clobber), and a "+" operand's input half repeats `{reg}`
// instead of tying by number unless it is early-clobber.
#if defined(__aarch64__)
long f(long a) {
  register long x0 asm("x0") = a;
  register long r asm("x1");
  asm volatile("mov %0, %1" : "=r"(r) : "r"(x0));
  return r;
}
long g(long a) {
  register long fp asm("x29") = a;
  register long w asm("x2");
  register int s asm("w3");
  asm volatile("# %0 %1 %w2" : "=&r"(w), "+r"(fp) : "r"((s)));
  return w + fp;
}
#else
long f(long a) {
  register long di asm("rdi") = a;
  register long r asm("rax");
  register int c asm("%ecx");
  asm volatile("mov %1, %0" : "=r"(r), "+r"(c) : "r"(di));
  return r + c;
}
long g() {
  register long e asm("r8");
  asm volatile("# %0" : "+&r"(e));
  return e;
}
#endif
