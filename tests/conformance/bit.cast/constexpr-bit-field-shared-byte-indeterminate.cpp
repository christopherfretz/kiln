// [bit.cast]/2: an `unsigned char` bit-field whose bits come from padding
// (here an unnamed bit-field, [class.bit]/2) holds an indeterminate value,
// and reading it is not constant ([expr.const]), even when another
// bit-field of the same byte is determinate. The Oven tracks
// initialization per byte, so the determinate neighbour makes the byte
// initialized and the read yields zero bits. GCC 16.1 rejects the read.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++20
//@ oracle: gcc
//@ xfail(kiln): WP-1573: an indeterminate bit-field sharing a byte with a determinate one reads as zero bits
//@ xfail(clang): clang 22 does not bit-cast bit-fields in constant evaluation
//@ diag: 20:15 error "static assertion expression is not an integral constant expression"
//@ diag: 19:73 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 20:15 note "in call to 'low()'"
struct Dst { unsigned char low : 5, high : 3; };
struct Src { unsigned char low : 1, : 4, high : 3; };
constexpr int low() { return __builtin_bit_cast(Dst, Src{1, 2}).low; }
static_assert(low() == 1);
