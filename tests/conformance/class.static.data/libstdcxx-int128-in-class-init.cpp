// [class.static.data]/4, [expr.const]: an in-class initializer of a
// `static const` data member of (unsigned) __int128 type is a constant
// expression. libstdc++ ext/numeric_traits.h:75 (__numeric_traits_integer
// <unsigned __int128>::__max = ~(_Value)0) cascading into
// bits/max_size_type.h:775 (numeric_limits<__max_size_type>::digits);
// 56 headers (docs/status/libstdcxx.md, WP-188).
// Verified against clang 19.
//@ expect: compile-ok
//@ xfail(kiln): in-class initializer of __int128 type rejected as not a constant expression
struct T { static const unsigned __int128 m = ~(unsigned __int128)0; static const unsigned __int128 n = (unsigned __int128)1 << 100; static const __int128 k = -(__int128)1;};
static_assert(T::m > 0);
