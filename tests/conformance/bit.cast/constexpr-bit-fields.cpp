// [bit.cast]/2: bit-fields are not in the "Constant When" exclusions, so
// a bit-cast from and to a class with bit-fields is a constant: each bit of
// the result's value representation is the corresponding bit of the
// source. The bits of a source byte outside every bit-field are padding
// (indeterminate), which only `unsigned char` or `std::byte` may receive;
// such a destination bit-field is itself the smallest object containing
// the bit, so an `unsigned char` bit-field may be left indeterminate.
// clang 22 rejects every bit-field ("not yet supported"); GCC 16.1 accepts.
// Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ strict-diags
//@ std: c++20
//@ oracle: gcc
//@ xfail(clang): clang 22 does not bit-cast bit-fields in constant evaluation
struct Fields { unsigned lo : 4, mid : 12, hi : 16; };
static_assert(__builtin_bit_cast(unsigned, Fields{0x1, 0x234, 0x5678}) == 0x56782341u);
constexpr Fields f = __builtin_bit_cast(Fields, 0xfedcba98u);
static_assert(f.lo == 0x8 && f.mid == 0xba9 && f.hi == 0xfedc);

// Signed bit-fields read back sign-extended.
struct Signed { int a : 3; int b : 29; };
constexpr Signed s = __builtin_bit_cast(Signed, -1);
static_assert(s.a == -1 && s.b == -1);
static_assert(__builtin_bit_cast(int, Signed{-4, 5}) == (5 << 3 | 4));
static_assert(__builtin_bit_cast(Signed, __builtin_bit_cast(int, Signed{2, -9})).b == -9);

// Two bit-fields filling one byte.
struct Byte { signed char x : 1; signed char y : 7; };
static_assert(__builtin_bit_cast(unsigned char, Byte{-1, 0}) == 1);
static_assert(__builtin_bit_cast(unsigned char, Byte{0, -1}) == 0xfe);
constexpr Byte by = __builtin_bit_cast(Byte, (unsigned char)0x81);
static_assert(by.x == -1 && by.y == -64);

// Bit-fields spanning bytes and an ordinary member after them.
struct Wide { unsigned long long a : 11, b : 3, c : 37, d : 13; unsigned long long e; };
struct Two { unsigned long long p, q; };
constexpr Wide w = __builtin_bit_cast(Wide, Two{~0ull, 42});
static_assert(w.a == 0x7ff && w.b == 7 && w.c == 0x1fffffffffull && w.d == 0x1fff && w.e == 42);
static_assert(__builtin_bit_cast(Two, Wide{1, 2, 3, 4, 5}).p ==
              (1ull | 2ull << 11 | 3ull << 14 | 4ull << 51));

// The padding bits of a source bit-field's byte land in an `unsigned char`.
struct Low { unsigned v : 4; unsigned char rest[3]; };
struct Bytes4 { unsigned char b[4]; };
// (A result holding indeterminate bits is no constexpr variable's value.)
constexpr bool low() {
  Bytes4 b = __builtin_bit_cast(Bytes4, Low{9, {1, 2, 3}});
  return b.b[1] == 1 && b.b[2] == 2 && b.b[3] == 3;
}
static_assert(low());

// A destination `unsigned char` bit-field made of padding is indeterminate;
// its neighbours are not.
struct Padded { unsigned char a; short s; };
struct Dest { unsigned char x : 8; unsigned char y : 8; short s; };
constexpr bool dest() {
  Dest d = __builtin_bit_cast(Dest, Padded{3, 4});
  return d.x == 3 && d.s == 4;
}
static_assert(dest());

// Base classes with bit-fields (the base's padding bits stay padding).
struct Base { unsigned short k : 9; };
struct Derived : Base { unsigned short j : 8; };
constexpr bool derived() {
  Bytes4 b = __builtin_bit_cast(Bytes4, Derived{{0x1ab}, 0x55});
  return b.b[0] == 0xab && b.b[2] == 0x55;
}
static_assert(sizeof(Derived) == 4 && derived());
