// [bit.cast]/2: the bits of the result that come from padding bits of
// the source are indeterminate; an `unsigned char` bit-field holding one
// has an indeterminate value, and reading it is not constant
// ([expr.const]), even when determinate bit-fields of the same bytes make
// them initialized: two indeterminate bit-fields in one byte, padding
// bits that span two bytes, and bit-fields of array elements. GCC 16.1
// rejects every read.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++20
//@ oracle: gcc
//@ xfail(clang): clang 22 does not bit-cast bit-fields in constant evaluation
//@ diag: 25:15 error "static assertion expression is not an integral constant expression"
//@ diag: 24:83 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 25:15 note "in call to 'two()'"
//@ diag: 30:15 error "static assertion expression is not an integral constant expression"
//@ diag: 29:84 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 30:15 note "in call to 'span()'"
//@ diag: 40:15 error "static assertion expression is not an integral constant expression"
//@ diag: 38:38 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 40:15 note "in call to 'elem()'"
struct S2 { unsigned char a : 2, : 2, b : 2, : 2; };
struct D2 { unsigned char a : 2, x : 2, b : 2, y : 2; };
constexpr int two() { D2 d = __builtin_bit_cast(D2, S2{1, 2}); return d.a + d.b + d.y; }
static_assert(two());

struct Sx { unsigned short a : 4, : 8, b : 4; };
struct Dx { unsigned char a : 4, x : 4; unsigned char y : 4, b : 4; };
constexpr int span() { Dx d = __builtin_bit_cast(Dx, Sx{1, 2}); return d.a + d.b + d.y; }
static_assert(span());

struct Dst { unsigned char low : 5, high : 3; };
struct Src { unsigned char low : 1, : 4, high : 3; };
struct A { Dst d[2]; };
struct AS { Src s[2]; };
constexpr int elem() {
  A a = __builtin_bit_cast(A, AS{{{1, 2}, {0, 3}}});
  return a.d[0].high + a.d[1].high + a.d[1].low;
}
static_assert(elem());
