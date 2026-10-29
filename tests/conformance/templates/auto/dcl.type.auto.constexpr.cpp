// [dcl.constexpr]/10 with [dcl.spec.auto]: a constexpr placeholder variable
// deduces then must be a constant expression; `constexpr` adds `const`.
//@ strict-diags
constexpr auto a = 2 * 3;
constexpr auto b = 1.5;
constexpr auto *s = "kiln";
constexpr decltype(auto) c = a;
static_assert(a == 6 && b == 1.5 && s[1] == 'i' && c == 6);
static_assert(__is_same(decltype(a), const int) && __is_same(decltype(s), const char *const));
static_assert(__is_same(decltype(c), const int));
