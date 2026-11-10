// [meta.reflection.substitute], [meta.reflection.traits] (P2996R13),
// [meta.unary.prop]/3: a trait asked from the initializer of a variable
// template specialization that substitute(^^V, args) instantiates fails
// as it does outside the substitution: is_copy_constructible_type of an
// incomplete class is not a constant expression, with the note naming
// the type (spec/reflection.md §4.3, WP-538; before it the call failed
// silently, with no note). The failure is in the initializer, not in the
// substitution of the arguments, so it is a hard error and extract reads
// an initializer that is not a constant expression. GCC 16.1 also rejects
// the program; kiln names door() after the substitution's notes.
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 38:41 error "constexpr variable 'copyable_v<^^Inc>' must be initialized by a constant expression"
//@ diag: 39:52 note "in instantiation of variable template specialization 'copyable_v<^^Inc>' requested here"
//@ diag: 39:52 note "while substituting template arguments into 'copyable_v' for std::meta::substitute"
//@ diag: 40:15 note "in call to 'door(^^Inc)'"
//@ diag: 37:42 note "invalid use of incomplete type 'Inc'"
//@ diag: 38:54 note "in call to 'copyable(^^Inc)'"
//@ diag: 38:54 error "call to consteval function 'copyable' is not a constant expression"
//@ diag: 37:42 note "invalid use of incomplete type 'Inc'"
//@ diag: 38:54 note "in call to 'copyable(^^Inc)'"
//@ diag: 40:15 error "static assertion expression is not an integral constant expression"
//@ diag: 39:38 note "initializer of 'copyable_v<^^Inc>' is not a constant expression"
//@ diag: 40:15 note "in call to 'door(^^Inc)'"
//@ diag: 38:41 note "declared here"
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta_traits.h"
#endif
using namespace std::meta;

struct Inc;
consteval bool copyable(info t) { return is_copy_constructible_type(t); }
template <info C> inline constexpr bool copyable_v = copyable(C);
consteval bool door(info t) { return extract<bool>(substitute(^^copyable_v, {reflect_constant(t)})); }
static_assert(door(^^Inc));
