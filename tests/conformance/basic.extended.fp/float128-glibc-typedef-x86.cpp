// [conv.fctptr], [over.ics.scs] (kiln/clang-compat extension, WP-1058):
// on x86-64, glibc's <bits/floatn.h> declares `typedef __float128
// _Float128;` for clang (`__GNUC__` < 13). clang has no `_Float128`
// keyword, so there `logf128` and friends take and return `__float128`.
// kiln keeps `_Float128` a distinct P1467 type and drops that typedef in
// a system header (spec/types.md "Extended floating-point types",
// decision (B) of WP-1000), but once it has seen it, function types that
// differ only in `_Float128` against `__float128` (through pointers,
// references and `_Complex`) convert by a function pointer conversion, so
// LLVM's ConstantFolding.cpp, which passes `logf128` to a
// `float128 (*)(float128)` parameter (`float128` being `__float128` under
// `__clang__`, llvm/Support/float128.h), compiles as under clang.
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-ok
//@ strict-diags
# 1 "/usr/include/x86_64-linux-gnu/bits/floatn.h" 1 3
typedef __float128 _Float128;
extern "C" _Float128 logf128(_Float128);
extern "C" _Float128 modff128(_Float128, _Float128 *);
extern "C" _Float128 frexpf128(_Float128, int *);
extern "C" void sincosf128(_Float128, _Float128 *, _Float128 *);
# 23 "float128-glibc-typedef-x86.cpp" 2
typedef __float128 float128;
float128 apply(float128 (*f)(float128), float128 v) { return f(v); }
float128 g(float128 v) { return apply(logf128, v); }
float128 h(float128 v) { return apply(&logf128, v); }

float128 (*p)(float128) = logf128;
float128 (&r)(float128) = logf128;
float128 (*m)(float128, float128 *) = modff128;
float128 (*fx)(float128, int *) = frexpf128;
void (*sc)(float128, float128 *, float128 *) = &sincosf128;
float128 (*sp)(float128) = static_cast<float128 (*)(float128)>(logf128);

// Overload resolution: the conversion is an exact match.
int pick(float128 (*)(float128)) { return 1; }
int pick(...) { return 2; }
int which = pick(logf128);
