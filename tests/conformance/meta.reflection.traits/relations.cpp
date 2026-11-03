// [meta.reflection.traits] P2996R13, [meta.rel]: is_same_type compares
// the represented types (an alias is its type: p4); the base, conversion,
// layout and interconvertibility relations are the builtin trait engine's;
// is_{nothrow_,}invocable{_r,}_type instantiate the TU's std::is_invocable
// etc. (spec/reflection-traits.md §1.2).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include <type_traits>
#include "kiln_meta_traits.h"
using namespace std::meta;

struct B {};
struct V {};
struct Der : B, virtual V {};
struct Priv : private B {};
struct L1 { int a; char b; };
struct L2 { int x; char y; };
struct Conv { operator int() const; };
struct NtConv { operator long() const noexcept; };
using Int = int;
int f(long);
struct Fn { void operator()(int) const noexcept; };

static_assert(is_same_type(^^Int, ^^int) && !is_same_type(^^int, ^^const int));
static_assert(is_base_of_type(^^B, ^^Der) && is_base_of_type(^^B, ^^Priv));
static_assert(is_base_of_type(^^B, ^^B) && !is_base_of_type(^^int, ^^int));
static_assert(is_virtual_base_of_type(^^V, ^^Der) && !is_virtual_base_of_type(^^B, ^^Der));
static_assert(is_convertible_type(^^Der*, ^^B*) && !is_convertible_type(^^Priv*, ^^B*));
static_assert(is_convertible_type(^^Conv, ^^int) && !is_convertible_type(^^int, ^^Conv));
static_assert(is_nothrow_convertible_type(^^NtConv, ^^long));
static_assert(!is_nothrow_convertible_type(^^Conv, ^^int));
static_assert(is_layout_compatible_type(^^L1, ^^L2) && !is_layout_compatible_type(^^L1, ^^B));
static_assert(is_pointer_interconvertible_base_of_type(^^B, ^^Der) == __is_pointer_interconvertible_base_of(B, Der));

static_assert(is_invocable_type(^^decltype(f), {^^int}) && !is_invocable_type(^^decltype(f), {^^B}));
static_assert(is_invocable_r_type(^^long, ^^decltype(f), {^^short}));
static_assert(!is_invocable_r_type(^^B, ^^decltype(f), {^^int}));
static_assert(is_nothrow_invocable_type(^^Fn, {^^int}) && !is_nothrow_invocable_type(^^decltype(f), {^^int}));
static_assert(is_nothrow_invocable_r_type(^^void, ^^Fn, {^^int}));
static_assert(is_invocable_type(^^int (B::*)(), {^^B&}));

static_assert(is_convertible_type(^^Conv, ^^int) == std::is_convertible_v<Conv, int>);
static_assert(is_invocable_type(^^Fn, {^^int}) == std::is_invocable_v<Fn, int>);
