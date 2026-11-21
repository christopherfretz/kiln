// [ACLE] The FP8 scalar type `__mfp8` (ACLE `mfloat8_t`; WP-1791), an
// opaque 8-bit storage type of clang 22's (clang 19 has none):
// spec/types.md §5.1. It has no arithmetic, comparison or conversion to
// or from another type (only `__builtin_bit_cast`), and no
// list-initialization; the traits are clang 22's.
//@ requires: clang>=22
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9-a+sve2+fp8
//@ strict-diags
//@ diag: 51:12 error "invalid operands to binary expression ('__mfp8' and '__mfp8')"
//@ diag: 52:12 error "invalid operands to binary expression ('__mfp8' and '__mfp8')"
//@ diag: 53:12 error "invalid operands to binary expression ('__mfp8' and '__mfp8')"
//@ diag: 54:9 error "C-style cast from '__mfp8' to 'int' is not allowed"
//@ diag: 55:9 error "C-style cast from 'int' to '__mfp8' is not allowed"
//@ diag: 56:10 error "cannot initialize a variable of type '__mfp8' with an rvalue of type 'float'"
//@ diag: 57:8 error "cannot initialize a variable of type 'bool' with an lvalue of type '__mfp8'"
//@ diag: 58:9 error "invalid argument type '__mfp8' to unary expression"
//@ diag: 59:9 error "invalid argument type '__mfp8' to unary expression"
//@ diag: 60:10 error "value of type '__mfp8' is not contextually convertible to 'bool'"
//@ diag: 61:9 error "cannot initialize a variable of type 'float' with an lvalue of type '__mfp8'"
//@ diag: 62:7 error "assigning to '__mfp8' from incompatible type 'int'"
//@ diag: 63:3 error "cannot increment value of type '__mfp8'"
//@ diag: 64:5 error "invalid operands to binary expression ('__mfp8' and '__mfp8')"
//@ diag: 65:12 error "illegal initializer type '__mfp8'"
//@ diag: 66:15 error "illegal initializer type '__mfp8'"
//@ diag: 67:12 error "illegal initializer type '__mfp8'"
typedef __attribute__((neon_vector_type(8))) __mfp8 m8x8;
typedef __attribute__((neon_vector_type(16))) __mfp8 m8x16;
struct S { __mfp8 a; int b; };
__mfp8 g;
__mfp8 garr[4];
static_assert(sizeof(__mfp8) == 1 && alignof(__mfp8) == 1);
static_assert(sizeof(S) == 8 && sizeof(m8x8) == 8 && sizeof(m8x16) == 16);
void sizes(__clang_svmfloat8x2_t, __clang_svmfloat8x3_t, __clang_svmfloat8x4_t);
void scalar(__mfp8 a, __mfp8 b) {
  // Allowed: copies, the same-type casts, __builtin_bit_cast,
  // value-initialization, aggregates holding one.
  __mfp8 c = a;
  c = b;
  (void)(__mfp8)a;
  (void)static_cast<__mfp8>(a);
  (void)reinterpret_cast<char &>(a);
  unsigned char u = __builtin_bit_cast(unsigned char, a);
  __mfp8 d = __builtin_bit_cast(__mfp8, u);
  __mfp8 w = __mfp8();
  S s{};
  S s2{a, 1};
  __mfp8 arr[2] = {};
  __mfp8 *p = &d + 1;
  // Errors.
  (void)(a + b);
  (void)(a == b);
  (void)(a < b);
  (void)(int)a;
  (void)(__mfp8)1;
  __mfp8 e = 1.0f;
  bool z = a;
  (void)!a;
  (void)-a;
  (void)(a ? 1 : 2);
  float f = a;
  a = 0;
  ++a;
  a += b;
  __mfp8 l1{};
  __mfp8 l2 = {};
  __mfp8 l3{a};
}
#define T(x) static_assert(x, #x);
#define F(x) static_assert(!(x), #x);
T(__is_trivially_copyable(__mfp8))
T(__is_object(__mfp8))
T(__is_trivially_constructible(__mfp8))
T(__is_trivially_destructible(__mfp8))
F(__is_pod(__mfp8))
F(__is_trivial(__mfp8))
F(__is_scalar(__mfp8))
F(__is_arithmetic(__mfp8))
F(__is_fundamental(__mfp8))
F(__is_standard_layout(__mfp8))
F(__is_compound(__mfp8))
F(__is_floating_point(__mfp8))
F(__is_integral(__mfp8))
F(__is_class(__mfp8))
F(__has_unique_object_representations(__mfp8))
T(__is_trivially_copyable(__SVMfloat8_t))
T(__is_same(decltype(__builtin_bit_cast(__mfp8, (char)0)), __mfp8))
