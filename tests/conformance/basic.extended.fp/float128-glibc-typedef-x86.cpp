// On x86-64, glibc's <bits/floatn.h> declares `typedef __float128
// _Float128;` for clang (`__GNUC__` < 13), so in clang `logf128` and
// friends take and return `__float128`. kiln keeps `_Float128` a distinct
// P1467 type and drops that typedef in a system header
// (spec/types.md "Extended floating-point types"), so `logf128` is not a
// `__float128 (*)(__float128)`. LLVM's ConstantFolding.cpp passes
// `logf128` to a `float128 (*)(float128)` parameter, `float128` being
// `__float128` under `__clang__` (llvm/Support/float128.h; x86_64 sweep,
// llvm_project). AArch64 is unaffected: clang defines no `__FLOAT128__`
// there.
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-ok
//@ xfail(kiln): WP-961 follow-up, glibc's x86-64 `typedef __float128 _Float128` is dropped so `_Float128` functions are not `__float128` ones (seen building llvm_project, ConstantFolding.cpp)
# 1 "/usr/include/x86_64-linux-gnu/bits/floatn.h" 1 3
typedef __float128 _Float128;
extern "C" _Float128 logf128(_Float128);
# 18 "float128-glibc-typedef-x86.cpp" 2
typedef __float128 float128;
float128 apply(float128 (*f)(float128), float128 v) { return f(v); }
float128 g(float128 v) { return apply(logf128, v); }
static_assert(__is_same(decltype(logf128(0)), __float128));
