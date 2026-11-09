// `__builtin_readcyclecounter()` (clang EmitBuiltinExpr,
// BI__builtin_readcyclecounter): a call of `i64 @llvm.readcyclecounter()`,
// declared `nocallback nofree nosync nounwind willreturn`, on every
// target (WP-1057).
//@ requires: clang>=22
//@ expect: ir
unsigned long long f() { return __builtin_readcyclecounter(); }
unsigned long long g() {
  unsigned long long a = __builtin_readcyclecounter();
  return __builtin_readcyclecounter() - a;
}
