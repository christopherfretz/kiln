// [basic.extended.fp], [lex.fcon]: the _Float16 extended floating-point
// type and the f16 literal suffix (kiln already predefines __FLT16_*__
// macros, so libstdc++ enables its _Float16 paths). libstdc++ format:3347,
// bits/charconv.h; <format>, <print>, <iostream>, 24 headers (docs/status/libstdcxx.md, WP-188).
// Verified against clang 19.
//@ expect: compile-ok
//@ xfail(kiln): unknown type name '_Float16'
_Float16 h = 1.5f16;
static_assert(sizeof(_Float16) == 2);
template<class T, class U> constexpr bool same = false;
template<class T> constexpr bool same<T, T> = true;
static_assert(!same<_Float16, float>);
