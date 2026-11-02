// [complex.syn], C23 7.3 (<complex.h>), [basic.extended.fp]: GCC's
// `__builtin_c*f128` family, which libstdc++ 14's <complex> names for
// `std::complex<_Float128>` where `_Float128` is not `long double`
// (x86-64): each takes and returns `_Complex _Float128` (`cabs`/`carg`
// return `_Float128`) and is a call of the glibc function it names
// (WP-365). clang 19/22 declare none of them.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 19/22 do not declare the __builtin_c*f128 family
template <class A, class B> constexpr bool same = false;
template <class A> constexpr bool same<A, A> = true;

using F = _Float128;
using C = __complex__ _Float128;

F abs_(C z) { return __builtin_cabsf128(z); }
F arg_(C z) { return __builtin_cargf128(z); }
C pow_(C x, C y) { return __builtin_cpowf128(x, y); }

C z;
static_assert(same<decltype(__builtin_cabsf128(z)), F>);
static_assert(same<decltype(__builtin_cargf128(z)), F>);
static_assert(same<decltype(__builtin_cacosf128(z)), C>);
static_assert(same<decltype(__builtin_cacoshf128(z)), C>);
static_assert(same<decltype(__builtin_casinf128(z)), C>);
static_assert(same<decltype(__builtin_casinhf128(z)), C>);
static_assert(same<decltype(__builtin_catanf128(z)), C>);
static_assert(same<decltype(__builtin_catanhf128(z)), C>);
static_assert(same<decltype(__builtin_ccosf128(z)), C>);
static_assert(same<decltype(__builtin_ccoshf128(z)), C>);
static_assert(same<decltype(__builtin_cexpf128(z)), C>);
static_assert(same<decltype(__builtin_clogf128(z)), C>);
static_assert(same<decltype(__builtin_cpowf128(z, z)), C>);
static_assert(same<decltype(__builtin_cprojf128(z)), C>);
static_assert(same<decltype(__builtin_csinf128(z)), C>);
static_assert(same<decltype(__builtin_csinhf128(z)), C>);
static_assert(same<decltype(__builtin_csqrtf128(z)), C>);
static_assert(same<decltype(__builtin_ctanf128(z)), C>);
static_assert(same<decltype(__builtin_ctanhf128(z)), C>);
