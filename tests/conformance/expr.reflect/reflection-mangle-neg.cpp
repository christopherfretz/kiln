// [basic.types.general]/12, [expr.const] P2996R13 (spec/reflection.md §4.1,
// §8.3): a reflection never reaches a symbol except as a template
// argument. A non-template variable of type std::meta::info with static
// storage duration (namespace scope, internal linkage, inline, static
// data member, function-local static, thread_local) that is not constexpr
// is rejected, so no such variable is emitted or mangled.
//@ expect: compile-fail
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 18:13 error "consteval-only variable 's' not declared 'constexpr'"
//@ diag: 19:13 error "consteval-only variable 'i' not declared 'constexpr'"
//@ diag: 20:19 error "consteval-only variable 't' not declared 'constexpr'"
//@ diag: 21:18 error "consteval-only variable 'anon' not declared 'constexpr'"
//@ diag: 22:31 error "consteval-only variable 'n' not declared 'constexpr'"
//@ diag: 23:26 error "consteval-only variable 'l' not declared 'constexpr'"
using info = decltype(^^int);
static info s = ^^int;
inline info i = ^^int;
thread_local info t;
namespace { info anon = ^^int; }
struct C { static inline info n = ^^int; };
int user() { static info l = ^^int; return 0; }
constexpr info ok = ^^int;   // constexpr: a compile-time value, never emitted
