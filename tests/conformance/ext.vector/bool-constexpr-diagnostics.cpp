// Clang ext vectors, bool element type ([bit.cast]; clang
// checkBitCastConstexprEligibilityType, HandleFloatToIntCast): a bool ext
// vector whose lane count is not a multiple of 8 cannot be bit-cast in a
// constant expression, in either direction, and __builtin_convertvector
// of a float lane outside the range of bool is not a constant
// expression. WP-450.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 37:14 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 32:33 note "bit_cast involving type 'bool __attribute__((ext_vector_type(4)))' (vector of 4 'bool' values) is not allowed in a constant expression; element size 1 * element count 4 is not a multiple of the byte size 8"
//@ diag: 37:18 note "in call to 'to4({true, false, true, true})'"
//@ diag: 38:14 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 33:35 note "bit_cast involving type 'bool __attribute__((ext_vector_type(4)))' (vector of 4 'bool' values) is not allowed in a constant expression; element size 1 * element count 4 is not a multiple of the byte size 8"
//@ diag: 38:18 note "in call to 'from4(1)'"
//@ diag: 39:15 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 34:36 note "bit_cast involving type 'bool __attribute__((ext_vector_type(12)))' (vector of 12 'bool' values) is not allowed in a constant expression; element size 1 * element count 12 is not a multiple of the byte size 8"
//@ diag: 39:19 note "in call to 'to12({true, false, false, false, false, false, false, false, false, false, false, false})'"
//@ diag: 40:15 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 35:38 note "bit_cast involving type 'bool __attribute__((ext_vector_type(12)))' (vector of 12 'bool' values) is not allowed in a constant expression; element size 1 * element count 12 is not a multiple of the byte size 8"
//@ diag: 40:19 note "in call to 'from12(1)'"
//@ diag: 43:14 error "constexpr variable 'e' must be initialized by a constant expression"
//@ diag: 43:18 note "value 2 is outside the range of representable values of type 'bool'"
//@ diag: 45:14 error "constexpr variable 'f' must be initialized by a constant expression"
//@ diag: 45:18 note "value -1 is outside the range of representable values of type 'bool'"
typedef bool b4 __attribute__((ext_vector_type(4)));
typedef bool b12 __attribute__((ext_vector_type(12)));
typedef float f4v __attribute__((ext_vector_type(4)));
typedef unsigned char u8;
typedef unsigned short u16;

constexpr u8 to4(b4 v) { return __builtin_bit_cast(u8, v); }
constexpr b4 from4(u8 v) { return __builtin_bit_cast(b4, v); }
constexpr u16 to12(b12 v) { return __builtin_bit_cast(u16, v); }
constexpr b12 from12(u16 v) { return __builtin_bit_cast(b12, v); }

constexpr u8 a = to4((b4){true, false, true, true});
constexpr b4 b = from4(1);
constexpr u16 c = to12((b12){true});
constexpr b12 d = from12(1);

constexpr f4v H = {0.0f, 2.0f, -0.5f, 0.0f};
constexpr b4 e = __builtin_convertvector(H, b4);
constexpr f4v N = {0.0f, 0.0f, -1.0f, 0.0f};
constexpr b4 f = __builtin_convertvector(N, b4);
