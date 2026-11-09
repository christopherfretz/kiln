// [lex.operators], [expr.reflect], [cpp.predefined] P2996R13: reflection is
// on by default at C++26 (operator decision Q7, WP-920): `^^` and `[: :]`
// work without -freflection, `__cpp_impl_reflection` is 202603L, and
// -freflection is a no-op (the last -f[no-]reflection wins; see
// flag-off-26.cpp).
//@ skip(clang): clang 19/22 do not implement P2996 reflection
//@ args: -fno-reflection -freflection
static_assert(__cpp_impl_reflection == 202603L);
using info = decltype(^^int);
constexpr info r = ^^int;
[:r:] x = 42;
static_assert(sizeof(x) == sizeof(int));
