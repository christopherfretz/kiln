// [meta.reflection.traits]/2 P2996R13, [meta.unary.prop]/3, [meta.trans.sign],
// [meta.trans.other] (spec/reflection-traits.md §2): a non-type reflection,
// an incomplete type where completeness is required, and an operand outside
// a transformation's domain make the call not a constant expression, with
// a note. A failure is never memoized: once Inc is complete the same call
// succeeds. A library-delegated trait whose std:: template the TU does not
// declare fails naming the header.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 41:15 error "static assertion expression is not an integral constant expression"
//@ diag: 41:15 note "invalid use of incomplete type 'Inc'"
//@ diag: 42:15 error "static assertion expression is not an integral constant expression"
//@ diag: 42:15 note "'^^v' does not represent a type"
//@ diag: 43:15 error "static assertion expression is not an integral constant expression"
//@ diag: 43:15 note "'^^std' does not represent a type"
//@ diag: 44:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:15 note "'make_signed' requires a non-bool integral or enumeration type, not 'bool'"
//@ diag: 45:15 error "static assertion expression is not an integral constant expression"
//@ diag: 45:15 note "'int' is not an enumeration type"
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 46:15 note "invalid use of incomplete type 'Tmpl<int>'"
//@ diag: 47:15 error "static assertion expression is not an integral constant expression"
//@ diag: 47:15 note "'is_swappable_type' needs 'std::is_swappable', which is not declared (include <type_traits>)"
//@ diag: 48:15 error "static assertion expression is not an integral constant expression"
//@ diag: 48:15 note "in call to 'is_constructible_type<std::initializer_list<info>>(^^Inc, {})'"
//@ diag: 49:15 error "static assertion expression is not an integral constant expression"
//@ diag: 49:15 note "kiln: 'type_order' is not implemented yet (it returns std::strong_ordering)"
#include "kiln_meta_traits.h"
using namespace std::meta;
namespace std::meta {
consteval int type_order(info a, info b);
}

struct Inc;
template <class T> struct Tmpl;
int v;
enum E { e };

static_assert(is_trivially_copyable_type(^^Inc));
static_assert(is_integral_type(^^v));
static_assert(is_class_type(^^std));
static_assert(make_signed(^^bool) == ^^int);
static_assert(underlying_type(^^int) == ^^int);
static_assert(is_empty_type(^^Tmpl<int>));
static_assert(is_swappable_type(^^int));
static_assert(is_constructible_type(^^Inc, {}));
static_assert(type_order(^^int, ^^E) == 0);

struct Inc {};
static_assert(is_trivially_copyable_type(^^Inc));
static_assert(is_empty_type(^^Inc));
