// [driver] The exclusive accesses lower as clang 19's
// `EmitAArch64BuiltinExpr`: `__builtin_arm_ldrex`/`ldaex` call
// `llvm.aarch64.ldxr`/`ldaxr` with an `elementtype` of the access width and
// narrow (truncate, then bitcast or `inttoptr`) the `i64` result, 128-bit
// ones use `ldxp`/`ldaxp`; `strex`/`stlex` widen the value to `i64` (or
// split it into two halves for `stxp`/`stlxp`) and return the status
// (spec/codegen.md "Target builtins"). WP-846.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
int f_ldrex8(char *p) { return __builtin_arm_ldrex(p); }
int f_ldrex16(short *p) { return __builtin_arm_ldrex(p); }
int f_ldrex32(const int *p) { return __builtin_arm_ldrex(p); }
long f_ldaex64(long *p) { return __builtin_arm_ldaex(p); }
__int128 f_ldrex128(__int128 *p) { return __builtin_arm_ldrex(p); }
double f_ldrexd(double *p) { return __builtin_arm_ldrex(p); }
float f_ldrexf(float *p) { return __builtin_arm_ldaex(p); }
int *f_ldrexp(int **p) { return __builtin_arm_ldrex(p); }
int f_strex8(char *p, char v) { return __builtin_arm_strex(v, p); }
int f_strex16(short *p) { return __builtin_arm_stlex(3, p); }
int f_strex64(long *p, long v) { return __builtin_arm_strex(v, p); }
int f_strex128(__int128 *p, __int128 v) { return __builtin_arm_stlex(v, p); }
int f_strexd(double *p, double v) { return __builtin_arm_strex(v, p); }
int f_strexf(float *p, float v) { return __builtin_arm_strex(v, p); }
int f_strexp(int **p, int *v) { return __builtin_arm_strex(v, p); }
bool f_ldrexb(bool *p) { return __builtin_arm_ldrex(p); }
int f_strexb(bool *p, bool v) { return __builtin_arm_strex(v, p); }
_BitInt(24) f_ldrex24(_BitInt(24) *p) { return __builtin_arm_ldaex(p); }
int f_strex24(_BitInt(24) *p, _BitInt(24) v) { return __builtin_arm_stlex(v, p); }
long double f_ldrexld(long double *p) { return __builtin_arm_ldrex(p); }
int f_strexld(long double *p, long double v) { return __builtin_arm_strex(v, p); }
__fp16 f_ldrexh(__fp16 *p) { return __builtin_arm_ldrex(p); }
int f_strexh(__fp16 *p) { return __builtin_arm_strex(1.0, p); }
