// GNU `__typeof` / `__typeof__` (the type of an expression, unevaluated,
// with no decltype reference rules): libstdc++ bits/c++locale.h:52
// (`extern "C" __typeof(uselocale) __uselocale;`), reached from <locale>,
// <codecvt>, <streambuf> and every iostream header (docs/status/libstdcxx.md,
// WP-188). Verified against clang 19.
//@ expect: compile-ok
//@ xfail(kiln): '__typeof' is not supported yet
extern "C" int f(int);
extern "C" __typeof(f) g;
int x;
__typeof__(x) y = 1;
__typeof((x)) z = 2;          // not a reference, unlike decltype((x))
__typeof(int *) p = &x;
static_assert(sizeof(__typeof(x + 1L)) == sizeof(long));
int h() { return g(y) + z + *p; }
