// [expr.const.const]/3 (P4101R1; spec/reflection.md §4.1, §8.3): a
// non-null reflection never reaches a symbol except as a template
// argument. A non-template variable of type std::meta::info with static
// storage duration (namespace scope, internal linkage, inline, static
// data member, function-local static) that holds one and is not
// constexpr is rejected, so no such variable is emitted or mangled. One
// holding the null reflection (`thread_local info t;`) is well-formed;
// codegen reports it "not supported yet" (WP-1328 land 2).
//@ expect: compile-fail
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 19:13 error "'s' is initialized with a consteval-only value but is not declared 'constexpr'"
//@ diag: 20:13 error "'i' is initialized with a consteval-only value but is not declared 'constexpr'"
//@ diag: 22:18 error "'anon' is initialized with a consteval-only value but is not declared 'constexpr'"
//@ diag: 23:31 error "'n' is initialized with a consteval-only value but is not declared 'constexpr'"
//@ diag: 24:26 error "'l' is initialized with a consteval-only value but is not declared 'constexpr'"
using info = decltype(^^int);
static info s = ^^int;
inline info i = ^^int;
thread_local info t;                // OK: the null reflection
namespace { info anon = ^^int; }
struct C { static inline info n = ^^int; };
int user() { static info l = ^^int; return 0; }
constexpr info ok = ^^int;   // constexpr: a compile-time value, never emitted
