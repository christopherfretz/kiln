// [meta.reflection.traits] P2996R13, [meta.unary.prop]: the supported
// operation traits. The constructible/assignable/destructible families,
// is_implicit_lifetime_type and reference_{constructs,converts}_from_temporary
// are the builtin trait engine's; is_{nothrow_,}swappable{_with,}_type have
// no builtin and instantiate the TU's std::is_swappable etc.
// (spec/reflection-traits.md §1.2). The range forms take a braced list of
// reflections (`R = initializer_list<info>`).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ xfail(gcc): GCC 16.1's is_copy_constructible_type(^^int&) is ill-formed (`const` applied to `int&`)
#include <type_traits>
#include <utility>
#include "kiln_meta_traits.h"
using namespace std::meta;

struct D { D() = default; };
struct NoDef { NoDef(int); };
struct NoCopy { NoCopy() = default; NoCopy(const NoCopy&) = delete; };
struct MoveOnly { MoveOnly() = default; MoveOnly(MoveOnly&&) = default; };
struct Throws { Throws(); Throws(const Throws&) noexcept(false); Throws& operator=(const Throws&); };
struct NtDtor { ~NtDtor() noexcept(false); };
struct UserDtor { ~UserDtor(); };
struct Two { Two(int, double); };

static_assert(is_constructible_type(^^Two, {^^int, ^^double}));
static_assert(!is_constructible_type(^^Two, {^^int}));
static_assert(is_constructible_type(^^int, {}) && is_constructible_type(^^NoDef, {^^int}));
static_assert(is_default_constructible_type(^^D) && !is_default_constructible_type(^^NoDef));
static_assert(is_copy_constructible_type(^^D) && !is_copy_constructible_type(^^NoCopy));
static_assert(is_move_constructible_type(^^MoveOnly) && !is_copy_constructible_type(^^MoveOnly));
static_assert(!is_copy_constructible_type(^^void) && is_copy_constructible_type(^^int&));
static_assert(is_assignable_type(^^int&, ^^int) && !is_assignable_type(^^int, ^^int));
static_assert(is_copy_assignable_type(^^D) && !is_copy_assignable_type(^^MoveOnly));
static_assert(is_move_assignable_type(^^D) && !is_move_assignable_type(^^const int));
static_assert(is_destructible_type(^^D) && !is_destructible_type(^^int[]));
static_assert(!is_destructible_type(^^void) && is_destructible_type(^^int&));

static_assert(is_trivially_constructible_type(^^int, {^^int}));
static_assert(!is_trivially_constructible_type(^^Two, {^^int, ^^double}));
static_assert(is_trivially_default_constructible_type(^^D));
static_assert(is_trivially_copy_constructible_type(^^D) && !is_trivially_copy_constructible_type(^^Throws));
static_assert(is_trivially_move_constructible_type(^^MoveOnly));
static_assert(is_trivially_assignable_type(^^int&, ^^int));
static_assert(is_trivially_copy_assignable_type(^^D) && !is_trivially_copy_assignable_type(^^Throws));
static_assert(is_trivially_move_assignable_type(^^D));
static_assert(is_trivially_destructible_type(^^D) && !is_trivially_destructible_type(^^UserDtor));

static_assert(is_nothrow_constructible_type(^^int, {^^long}));
static_assert(!is_nothrow_constructible_type(^^Throws, {}));
static_assert(is_nothrow_default_constructible_type(^^D));
static_assert(!is_nothrow_copy_constructible_type(^^Throws) && is_nothrow_copy_constructible_type(^^D));
static_assert(is_nothrow_move_constructible_type(^^MoveOnly));
static_assert(is_nothrow_assignable_type(^^int&, ^^int));
static_assert(!is_nothrow_copy_assignable_type(^^Throws) && is_nothrow_move_assignable_type(^^D));
static_assert(is_nothrow_destructible_type(^^D) && !is_nothrow_destructible_type(^^NtDtor));

static_assert(is_implicit_lifetime_type(^^int) && is_implicit_lifetime_type(^^D));
static_assert(!is_implicit_lifetime_type(^^UserDtor));
static_assert(reference_constructs_from_temporary(^^const int&, ^^int));
static_assert(!reference_constructs_from_temporary(^^int&, ^^int&));
static_assert(reference_converts_from_temporary(^^const long&, ^^int));

static_assert(is_swappable_type(^^int) && !is_swappable_type(^^NoCopy));
static_assert(is_swappable_with_type(^^int&, ^^int&) && !is_swappable_with_type(^^int, ^^int));
static_assert(is_nothrow_swappable_type(^^D));
static_assert(is_nothrow_swappable_with_type(^^int&, ^^int&));

static_assert(is_copy_constructible_type(^^NoCopy) == std::is_copy_constructible_v<NoCopy>);
static_assert(is_nothrow_destructible_type(^^NtDtor) == __is_nothrow_destructible(NtDtor));
