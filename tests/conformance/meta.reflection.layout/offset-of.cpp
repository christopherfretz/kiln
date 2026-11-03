// [meta.reflection.layout] (P2996R13): offset_of(r) is a non-static data
// member's offset in its class, as member_offset{bytes, bits} (bits < 8
// except on a bit-field); it agrees with offsetof wherever offsetof
// applies. The classes are the `tests/layout` corpus's (scalars_padding,
// nested_classes, access_mix, no_unique_address, bitfields_basic,
// bitfields_unnamed), so the two layout paths are checked against each
// other; the classes avoid every type whose layout differs between the
// supported targets. The bit-field block is the IS example.
//@ args: -freflection -I tests/support -Wno-invalid-offsetof
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
#include <cstddef>
#include <cstdint>
using namespace std::meta;
constexpr auto uc = access_context::unchecked();

consteval std::size_t nth_offset(info cls, std::size_t i) {
  auto o = offset_of(nonstatic_data_members_of(cls, uc)[i]);
  return o.bytes;
}
consteval long nth_bits(info cls, std::size_t i) {
  auto o = offset_of(nonstatic_data_members_of(cls, uc)[i]);
  return 8 * o.bytes + o.bits;
}

struct C { int a; char b; short c; double d; };
struct S { C c; char p; };
static_assert(offset_of(^^C::a).bytes == offsetof(C, a));
static_assert(offset_of(^^C::b).bytes == offsetof(C, b));
static_assert(offset_of(^^C::c).bytes == offsetof(C, c));
static_assert(offset_of(^^C::d).bytes == offsetof(C, d));
static_assert(offset_of(^^C::d).bits == 0);
static_assert(offset_of(^^S::p).bytes == offsetof(S, p));
static_assert(size_of(^^S) == sizeof(S) && alignment_of(^^S) == alignof(S));

// Access does not change the layout query.
class Mix {
  char a;
 public:
  int b;
 protected:
  char c;
 public:
  static consteval std::size_t ca() { return offsetof(Mix, a); }
  static consteval std::size_t cc() { return offsetof(Mix, c); }
};
static_assert(nth_offset(^^Mix, 0) == Mix::ca());
static_assert(nth_offset(^^Mix, 1) == offsetof(Mix, b));
static_assert(nth_offset(^^Mix, 2) == Mix::cc());

// [[no_unique_address]] members may share an address.
struct E {};
struct N { [[no_unique_address]] E e; int i; };
struct J { int i; [[no_unique_address]] E e; };
static_assert(offset_of(^^N::e).bytes == offsetof(N, e));
static_assert(offset_of(^^N::i).bytes == offsetof(N, i));
static_assert(offset_of(^^J::e).bytes == offsetof(J, e));
static_assert(size_of(^^N::e) == sizeof(E));

// Bit-fields (offsetof does not apply): bits counts from the byte.
struct B { int a : 3; int b : 5; int c : 30; char d : 2; };
static_assert(nth_bits(^^B, 0) == 0 && nth_bits(^^B, 1) == 3);
static_assert(nth_bits(^^B, 2) == 32 && nth_bits(^^B, 3) == 62);
static_assert(offset_of(^^B::b).bytes == 0 && offset_of(^^B::b).bits == 3);
static_assert(bit_size_of(^^B::c) == 30 && bit_size_of(^^B::d) == 2);
static_assert(size_of(^^B) == sizeof(B));

// An unnamed bit-field is not a member, but the zero-width one moves c.
struct U { int : 3; int a : 5; int : 0; char c; short : 4; };
static_assert(nonstatic_data_members_of(^^U, uc).size() == 2);
static_assert(offset_of(^^U::a).bits == 3 && offset_of(^^U::c).bytes == offsetof(U, c));

// The IS example.
struct Msg {
    uint64_t a : 10, b : 8, c : 4, d : 42;
};
static_assert(offset_of(^^Msg::a).bytes == 0);
static_assert(offset_of(^^Msg::b).bytes == 1);
static_assert(offset_of(^^Msg::c).bytes == 2);
static_assert(offset_of(^^Msg::d).bytes == 2);
static_assert(offset_of(^^Msg::a).bits == 0);
static_assert(offset_of(^^Msg::b).bits == 2);
static_assert(offset_of(^^Msg::c).bits == 2);
static_assert(offset_of(^^Msg::d).bits == 6);

// Direct bases: the offset of the base subobject; a virtual base's in a
// complete object of the derived class (after the vptr and the
// non-virtual part).
struct B1 { char c; };
struct B2 { int i; };
struct V { short s[3]; };
struct ND : B1, B2 { char own; };
static_assert(offset_of(bases_of(^^ND, uc)[0]).bytes == 0);
static_assert(offset_of(bases_of(^^ND, uc)[1]).bytes == alignof(B2));
static_assert(offset_of(^^ND::own).bytes == offsetof(ND, own));
struct D : B1, B2, virtual V { char own; };
static_assert(offset_of(bases_of(^^D, uc)[0]).bytes == sizeof(void*));
static_assert(offset_of(bases_of(^^D, uc)[1]).bytes == sizeof(void*) + alignof(B2));
static_assert(size_of(bases_of(^^D, uc)[1]) == sizeof(B2));
static_assert(alignment_of(bases_of(^^D, uc)[1]) == alignof(B2));
static_assert(bit_size_of(bases_of(^^D, uc)[2]) == 8 * sizeof(V));
static_assert(offset_of(bases_of(^^D, uc)[2]).bytes + sizeof(V) <= sizeof(D));
static_assert(offset_of(bases_of(^^D, uc)[2]).bytes >= offsetof(D, own) + 1);
static_assert(offset_of(^^D::own).bytes == offsetof(D, own));

int main() {}
