// [meta.reflection.layout]: size_of, alignment_of and bit_size_of of a
// data member description (T, N, A, W, NUA) measure T (alignment_of is A
// when given, bit_size_of is W when given, size_of and alignment_of
// reject one with a W); of an object reflection the object (not the
// pointer that designates it), with a variable's own alignment for its
// complete object; of a function parameter in its function's body the
// parameter variable; alignment_of of a variable of incomplete array
// type its element's. A value has no alignment. [meta.reflection.names]:
// a description's identifier is its N. [meta.reflection.queries]:
// is_bit_field of a description is whether W is given. Found running
// GCC's C++ testsuite.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __KILN__
#include "kiln_meta.h"
namespace std::meta {
consteval bool is_bit_field(info r);
}
#else
#include <meta>
#endif
using namespace std::meta;

consteval bool has_size_of(info r) {
  try { size_of(r); } catch (std::meta::exception &) { return false; }
  return true;
}
consteval bool has_alignment_of(info r) {
  try { alignment_of(r); } catch (std::meta::exception &) { return false; }
  return true;
}

constexpr info plain = data_member_spec(^^short, {.name = "m"});
constexpr info aligned = data_member_spec(^^int, {.name = "a", .alignment = 16});
constexpr info bits = data_member_spec(^^int, {.name = "b", .bit_width = 6});
constexpr info anon = data_member_spec(^^int, {.bit_width = 0});

static_assert(size_of(plain) == sizeof(short));
static_assert(alignment_of(plain) == alignof(short));
static_assert(bit_size_of(plain) == 8 * sizeof(short));
static_assert(alignment_of(aligned) == 16 && size_of(aligned) == sizeof(int));
static_assert(bit_size_of(bits) == 6);
static_assert(!has_size_of(bits) && !has_alignment_of(bits));
static_assert(is_bit_field(bits) && is_bit_field(anon) && !is_bit_field(plain));
static_assert(has_identifier(plain) && identifier_of(plain) == "m");
static_assert(!has_identifier(anon));

alignas(32) int over[3];
int arr[4];
extern int unbounded[];
static_assert(size_of(reflect_object(arr)) == sizeof(arr));
static_assert(alignment_of(reflect_object(arr)) == alignment_of(^^arr));
static_assert(alignment_of(reflect_object(arr[1])) == alignof(int));
static_assert(size_of(reflect_object(arr[1])) == sizeof(int));
static_assert(alignment_of(reflect_object(over)) == 32);
static_assert(alignment_of(^^unbounded) == alignof(int));
static_assert(!has_alignment_of(reflect_constant(42)));

int f(int a, const long b, double c[2]) {
  static_assert(size_of(^^a) == sizeof(int) && alignment_of(^^a) == alignof(int));
  static_assert(bit_size_of(^^b) == 8 * sizeof(long));
  static_assert(size_of(^^c) == sizeof(double *));
  return a;
}
