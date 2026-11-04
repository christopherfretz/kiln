// The multiprecision carry builtins (WP-557; clang EmitBuiltinExpr
// BI__builtin_addcb..): two llvm.u{add,sub}.with.overflow steps, the flags
// OR-ed and zero-extended into the carry-out, every width.
unsigned char f1(unsigned char a, unsigned char b, unsigned char c, unsigned char *o) {
  return __builtin_addcb(a, b, c, o);
}
unsigned short f2(unsigned short a, unsigned short b, unsigned short c, unsigned short *o) {
  return __builtin_subcs(a, b, c, o);
}
unsigned f3(unsigned a, unsigned b, unsigned c, unsigned *o) { return __builtin_addc(a, b, c, o); }
unsigned long f4(unsigned long a, unsigned long b, unsigned long c, unsigned long *o) {
  return __builtin_subcl(a, b, c, o);
}
unsigned long long f5(unsigned long long a, unsigned long long b, unsigned long long c,
                      unsigned long long *o) {
  return __builtin_addcll(a, b, c, o);
}
unsigned long long f6(unsigned long long a, int b, unsigned long long *o) {
  return __builtin_subcll(a, b, 1, o);
}
