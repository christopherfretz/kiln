// [expr.call]: the declarations of the LLVM intrinsics that builtin calls
// lower to carry LLVM 22's `nocreateundeforpoison` exactly where LLVM 22
// marks the intrinsic `IntrNoCreateUndefOrPoison` (spec/llvm.md §6):
// with.overflow, ctpop, bswap, bitreverse, fshl/fshr, fabs, copysign,
// minnum/maxnum, fma, fmuladd, the rounding functions, smax/smin/umax/umin
// and the saturating additions, but not abs, ctlz or cttz. Compared with
// clang 22's IR exactly.
//@ expect: ir
// clang 22 has no C++26 erroneous values (P2795R5, spec/codegen.md §7r).
//@ args: -ftrivial-auto-var-init=uninitialized
//@ requires: clang>=22
//@ requires: llvm>=22
int overflow(int x, int y) {
  int r;
  return __builtin_add_overflow(x, y, &r) + __builtin_mul_overflow(x, y, &r)
       + __builtin_sub_overflow(x, y, &r) + r;
}
unsigned bits(unsigned x, unsigned y) {
  return __builtin_popcount(x) + __builtin_clz(x) + __builtin_ctz(x)
       + __builtin_bswap32(x) + __builtin_bitreverse32(x)
       + __builtin_rotateleft32(x, y) + __builtin_rotateright32(x, y);
}
int absolute(int x) { return __builtin_abs(x); }
double fp(double x, double y) {
  return __builtin_fabs(x) + __builtin_copysign(x, y) + __builtin_fmin(x, y)
       + __builtin_fmax(x, y) + __builtin_fma(x, y, x) + __builtin_floor(x)
       + __builtin_ceil(x) + __builtin_trunc(x) + __builtin_round(x)
       + __builtin_rint(x) + __builtin_nearbyint(x);
}
double contract(double a, double b, double c) { return a * b + c; }
