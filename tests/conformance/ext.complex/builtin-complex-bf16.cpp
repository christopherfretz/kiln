// `__builtin_complex` with `__bf16` operands gives `_Complex __bf16`, as
// clang 19 and 22 do (both reject the spelled type `_Complex __bf16`,
// which kiln accepts for libstdc++: complex-bf16.cpp), and folds; operand
// errors as clang words them (WP-380; spec/sema.md §5.16).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:28 error "arguments are of different types ('B' (aka '__bf16') vs 'float')"
//@ diag: 20:28 error "arguments are of different types ('B' (aka '__bf16') vs '_Float16')"
//@ diag: 21:28 error "argument type 'int' is not a real floating point type"
using B = __bf16;
constexpr auto a = __builtin_complex((B)1.5f, (B)-2);
static_assert(__is_same(decltype(a), decltype(__builtin_complex((B)0, (B)0)) const));
static_assert(__is_same(decltype(__real__ (a + a)), __bf16));
static_assert(sizeof(a) == 4 && alignof(decltype(a)) == 2);
static_assert(__real__ a == (B)1.5f && __imag__ a == (B)-2);
constexpr auto p = a * __builtin_complex((B)2, (B)0);
static_assert(__real__ p == (B)3 && __imag__ p == (B)-4);
static_assert(a == a && a != p);
auto c = __builtin_complex((B)1, 2.0f);
auto d = __builtin_complex((B)1, (_Float16)2);
auto e = __builtin_complex(1, 2);
