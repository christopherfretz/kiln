// [dcl.asm] (GNU extension): a condition-flag output `=@cc<cond>` becomes
// the `{@cc<cond>}` constraint; its value is 0 or 1, so each one is
// followed by `icmp ult ..., 2` and `llvm.assume` before the store.
#if defined(__aarch64__)
int eq(int a, int b) {
  bool z;
  asm("cmp %w1, %w2" : "=@cceq"(z) : "r"(a), "r"(b));
  return z;
}
int two(long a) {
  int ne;
  char hs;
  asm("cmp %1, #0" : "=@ccne"(ne), "=@cchs"(hs) : "r"(a));
  return ne + hs;
}
#else
int eq(int a, int b) {
  bool z;
  asm("cmp %1, %2" : "=@ccz"(z) : "r"(a), "r"(b));
  return z;
}
int two(long a) {
  int ne;
  char ae;
  asm("test %1, %1" : "=@ccnz"(ne), "=@ccae"(ae) : "r"(a));
  return ne + ae;
}
#endif
