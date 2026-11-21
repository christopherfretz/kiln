// [bit.cast]/2, [class.bit]/2, [expr.const]: the padding bits of a
// constexpr variable's value (an unnamed bit-field, the unused high bits
// of a storage unit) stay indeterminate when a later constant expression
// bit-casts the variable: the value store keeps no bit-level padding, and
// the Oven reads a bit-field object member by member, so those bits are
// never read as zero. The determinate bits are read as usual.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++20
//@ oracle: gcc
//@ xfail(clang): clang 22 does not bit-cast bit-fields in constant evaluation
//@ diag: 23:15 error "static assertion expression is not an integral constant expression"
//@ diag: 22:30 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 23:15 note "in call to 'low()'"
struct Dst { unsigned char low : 5, high : 3; };
struct Src { unsigned char low : 1, : 4, high : 3; };
constexpr Src src{1, 2};
struct Wide { unsigned char a : 2; unsigned char b; };
struct Bytes { unsigned char x, y; };
constexpr Wide wide{3, 9};
static_assert(__builtin_bit_cast(Bytes, wide).y == 9);
constexpr int low() { return __builtin_bit_cast(Dst, src).low; }
static_assert(low() == 1);
static_assert(__builtin_bit_cast(Dst, src).high == 2);
