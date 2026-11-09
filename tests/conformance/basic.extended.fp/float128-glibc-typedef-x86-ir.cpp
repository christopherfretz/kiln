// [conv.fctptr] (kiln/clang-compat extension, WP-1058): after glibc's
// x86-64 `typedef __float128 _Float128;`, a `_Float128` function converts
// to a pointer to the same function spelled with `__float128`; the
// conversion is a no-op and the call passes and returns `fp128`, as in
// clang (where the two are one type). spec/types.md "Extended
// floating-point types".
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu
# 1 "/usr/include/x86_64-linux-gnu/bits/floatn.h" 1 3
typedef __float128 _Float128;
extern "C" _Float128 logf128(_Float128);
extern "C" _Float128 modff128(_Float128, _Float128 *);
# 13 "float128-glibc-typedef-x86-ir.cpp" 2
typedef __float128 float128;
float128 apply(float128 (*f)(float128), float128 v) { return f(v); }
float128 g(float128 v) { return apply(logf128, v); }
float128 split(float128 v, float128 *ip) {
  float128 (*m)(float128, float128 *) = modff128;
  return m(v, ip);
}
