// GCC types the `__builtin_*f128` family `_Float128`; libstdc++'s `<cmath>`
// `_Float128` overloads (taken on x86-64, where `long double` is x87) pass
// `_Float128*` to `modff128`, `frexpf128` and `remquof128`. A `__float128`
// value still converts both ways (same format). Agrees with GCC 14 (whose
// AArch64 port has no `__float128`, so the `g` line needs an x86-64 GCC).
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ xfail(clang): clang 19 types the f128 builtins `__float128` (x86-64) or `long double` (AArch64)
template<class T, class U> constexpr bool same = false;
template<class T> constexpr bool same<T, T> = true;

_Float128 m(_Float128 x, _Float128 *p) { return __builtin_modff128(x, p); }
_Float128 fr(_Float128 x, int *e) { return __builtin_frexpf128(x, e); }
_Float128 rq(_Float128 x, _Float128 y, int *q) { return __builtin_remquof128(x, y, q); }
_Float128 n() { return __builtin_nanf128(""); }
__float128 g(__float128 x) { return __builtin_sqrtf128(x); }

_Float128 x;
static_assert(same<decltype(__builtin_sqrtf128(x)), _Float128>);
static_assert(same<decltype(__builtin_inff128()), _Float128>);
static_assert(same<decltype(__builtin_fabsf128(x)), _Float128>);
static_assert(__builtin_fabsf128(-2.5f128) == 2.5f128);
