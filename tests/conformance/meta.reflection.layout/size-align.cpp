// [meta.reflection.layout] (P2996R13): size_of, alignment_of and
// bit_size_of of a type, an object, a variable, a non-static data member
// or a direct base agree with sizeof/alignof (a member's alignas and a
// variable's count); a reference member measures as a pointer
// ([meta.reflection.layout]/7); bit_size_of a bit-field is its width.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;

struct alignas(16) Over { char c; };
struct T {
  char c;
  alignas(8) short s;
  int& r;
  Over o;
  unsigned w : 7;
};
using Alias = T;
static_assert(size_of(^^T) == sizeof(T) && alignment_of(^^T) == alignof(T));
static_assert(size_of(^^Alias) == sizeof(T));
static_assert(size_of(^^T::c) == 1 && alignment_of(^^T::c) == 1);
static_assert(size_of(^^T::s) == sizeof(short) && alignment_of(^^T::s) == 8);
static_assert(size_of(^^T::r) == sizeof(int*) && alignment_of(^^T::r) == alignof(int*));
static_assert(size_of(^^T::o) == 16 && alignment_of(^^T::o) == 16);
static_assert(bit_size_of(^^T::w) == 7 && bit_size_of(^^T::c) == 8);
static_assert(bit_size_of(^^T) == 8 * sizeof(T));
static_assert(size_of(^^int[5]) == sizeof(int[5]) && alignment_of(^^int[5]) == alignof(int));
static_assert(size_of(^^int*) == sizeof(int*));

alignas(32) int v;
int arr[7];
static_assert(size_of(^^v) == sizeof(int) && alignment_of(^^v) == 32);
static_assert(size_of(^^arr) == sizeof(arr));

enum class En : short { a };
static_assert(size_of(^^En) == sizeof(short));
static_assert(alignment_of(^^En) == alignof(short));

int main() {}
