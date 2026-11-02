// `__builtin_complex` with `_Float16` or `__fp16` operands gives
// `_Complex _Float16` / a complex `__fp16` from clang 22 (clang 19
// rejects both: builtin-complex-half-19.cpp), and folds (WP-380;
// spec/sema.md §5.16).
//@ expect: compile-ok
//@ requires: clang>=22
//@ strict-diags
auto b = __builtin_complex((_Float16)1, (_Float16)2);
static_assert(__is_same(decltype(b), _Complex _Float16));
auto h = __builtin_complex((__fp16)1, (__fp16)2);
static_assert(sizeof(h) == 4 && __is_same(decltype(__real__ (h + h)), __fp16));
constexpr auto f = __builtin_complex((_Float16)1.5f, (_Float16)-2) * __builtin_complex((_Float16)2, (_Float16)0);
static_assert(__real__ f == 3 && __imag__ f == -4);
