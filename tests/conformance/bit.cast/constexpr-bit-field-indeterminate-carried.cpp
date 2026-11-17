// [bit.cast]/2: `Dst::low` of the result takes four padding bits of the
// source, so it holds an indeterminate value while `Dst::high`, in the
// same byte, is determinate. The value stays indeterminate after the
// neighbour is assigned and through copies of the object (a bit-cast, a
// union member, `__builtin_memcpy`, a trivial copy), and a constant
// expression may not read it ([expr.const]) or hold it ([expr.const]:
// a constexpr variable's value). An `unsigned char` may be initialized
// from it, a `signed char` may not ([bit.cast]/2; the note lists plain `char` where it is unsigned). GCC 16.1 reads zero bits after the first copy.
//@ args: --target=aarch64-unknown-linux-gnu
//@ expect: compile-fail
//@ strict-diags
//@ std: c++20
//@ xfail(clang): clang 22 does not bit-cast bit-fields in constant evaluation
//@ diag: 45:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:62 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 45:15 note "in call to 'neighbour()'"
//@ diag: 48:15 error "static assertion expression is not an integral constant expression"
//@ diag: 47:47 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 48:15 note "in call to 'recast()'"
//@ diag: 52:15 error "static assertion expression is not an integral constant expression"
//@ diag: 51:49 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 52:15 note "in call to 'member()'"
//@ diag: 56:15 error "static assertion expression is not an integral constant expression"
//@ diag: 55:77 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 56:15 note "in call to 'byte()'"
//@ diag: 59:15 error "static assertion expression is not an integral constant expression"
//@ diag: 58:46 note "indeterminate value can only initialize an object of type 'unsigned char', 'char', or 'std::byte'; 'signed char' is invalid"
//@ diag: 59:15 note "in call to 'schar()'"
//@ diag: 62:15 error "static assertion expression is not an integral constant expression"
//@ diag: 61:86 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 62:15 note "in call to 'memcpy_()'"
//@ diag: 65:15 error "static assertion expression is not an integral constant expression"
//@ diag: 64:46 note "subobject 'low' is not initialized"
//@ diag: 64:46 note "in call to 'Dst(d)'"
//@ diag: 65:15 note "in call to 'copy()'"
//@ diag: 40:28 note "subobject declared here"
//@ diag: 67:15 error "constexpr variable 'var' must be initialized by a constant expression"
//@ diag: 67:15 note "subobject 'low' is not initialized"
//@ diag: 40:28 note "subobject declared here"
struct Dst { unsigned char low : 5, high : 3; };
struct Src { unsigned char low : 1, : 4, high : 3; };
constexpr Dst mk() { return __builtin_bit_cast(Dst, Src{1, 2}); }

constexpr int neighbour() { Dst d = mk(); d.high = 1; return d.low; }
static_assert(neighbour());

constexpr int recast() { Dst d = mk(); return __builtin_bit_cast(Dst, d).low; }
static_assert(recast());

union U { Dst d; int i; };
constexpr int member() { U u{.d = mk()}; return u.d.low; }
static_assert(member());

struct B { unsigned char c; };
constexpr int byte() { Dst d = mk(); B b = __builtin_bit_cast(B, d); return b.c; }
static_assert(byte());

constexpr int schar() { Dst d = mk(); return __builtin_bit_cast(signed char, d); }
static_assert(schar());

constexpr int memcpy_() { Dst d = mk(), e{7, 1}; __builtin_memcpy(&e, &d, 1); return e.low; }
static_assert(memcpy_());

constexpr int copy() { Dst d = mk(); Dst e = d; return e.high; }
static_assert(copy());

constexpr Dst var = mk();
