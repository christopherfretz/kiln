// [meta.reflection.substitute], [meta.reflection.traits] (P2996R13),
// [meta.unary.prop]: the assignability traits (is_copy_assignable_type,
// is_move_assignable_type, is_trivially_copy_assignable_type,
// is_nothrow_move_assignable_type: a trial assignment of
// add_lvalue_reference_t<T> from the implied argument) and the
// destructibility traits (is_destructible_type,
// is_nothrow_destructible_type, is_trivially_destructible_type) asked from
// the initializer of a variable template specialization that
// substitute(^^V, args) instantiates: true and false answers, as outside
// the substitution (spec/reflection.md §4.3, WP-538). Compared with GCC 16.1.
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta_traits.h"
#endif
using namespace std::meta;

struct Plain { int i; };
struct NoAssign { NoAssign& operator=(const NoAssign&) = delete; };
struct ThrowAssign { ThrowAssign& operator=(ThrowAssign&&) noexcept(false); };
struct NoDtor { ~NoDtor() = delete; };
struct HiddenDtor { private: ~HiddenDtor(); };
struct ThrowDtor { ~ThrowDtor() noexcept(false); };
struct UserDtor { ~UserDtor(); };

consteval unsigned assign_bits(info t) {
  return unsigned(is_copy_assignable_type(t)) | unsigned(is_move_assignable_type(t)) << 1 |
         unsigned(is_trivially_copy_assignable_type(t)) << 2 |
         unsigned(is_nothrow_move_assignable_type(t)) << 3;
}
consteval unsigned destroy_bits(info t) {
  return unsigned(is_destructible_type(t)) | unsigned(is_nothrow_destructible_type(t)) << 1 |
         unsigned(is_trivially_destructible_type(t)) << 2;
}
template <info C> inline constexpr unsigned assign_v = assign_bits(C);
template <info C> inline constexpr unsigned destroy_v = destroy_bits(C);
consteval unsigned assign_door(info t) {
  return extract<unsigned>(substitute(^^assign_v, {reflect_constant(t)}));
}
consteval unsigned destroy_door(info t) {
  return extract<unsigned>(substitute(^^destroy_v, {reflect_constant(t)}));
}

static_assert(assign_door(^^Plain) == 0b1111);
static_assert(assign_door(^^int) == 0b1111);
static_assert(assign_door(^^const int) == 0b0000);
static_assert(assign_door(^^NoAssign) == 0b0000);
static_assert(assign_door(^^ThrowAssign) == 0b0010);

static_assert(destroy_door(^^Plain) == 0b111);
static_assert(destroy_door(^^int&) == 0b111);
static_assert(destroy_door(^^void) == 0b000);
static_assert(destroy_door(^^NoDtor) == 0b000);
static_assert(destroy_door(^^HiddenDtor) == 0b000);
static_assert(destroy_door(^^ThrowDtor) == 0b001);
static_assert(destroy_door(^^UserDtor) == 0b011);

int main() {}
