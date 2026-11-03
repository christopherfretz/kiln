//@ min-oracle: 22
// WP-432: clang 20+ constant-evaluates __builtin_abs / _labs / _llabs, so
// `EmitBuiltinExpr` keeps the folded value; clang 19 emits `llvm.abs`
// (builtins_abs.cpp).

static_assert(__builtin_abs(-3) == 3);
static_assert(__builtin_labs(-3L) == 3);
static_assert(__builtin_llabs(-3LL) == 3);
constexpr int k = __builtin_abs(-7);
int f() { return __builtin_abs(-3); }
long g() { return __builtin_labs(-3); }
long long g2() { return __builtin_llabs(-9); }
int h(int x) { return __builtin_abs(x); }
