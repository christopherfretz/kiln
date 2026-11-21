// [ACLE] Operators on the FP8 vectors (WP-1791, clang 22's; clang 19 has
// no FP8 type): spec/types.md §5.1. The SVE `__SVMfloat8_t` (ACLE
// `svmfloat8_t`) takes the SVE integer vector operators `+ - * /`, `<`,
// unary `-` and subscripts but no bitwise, shift, `!` or `?:` operator, no
// scalar or other vector operand, and is sizeless; a NEON vector of
// `__mfp8` takes unary `-` and subscripts but no binary operator.
//@ requires: clang>=22
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9-a+sve2+fp8
//@ strict-diags
//@ diag: 35:12 error "invalid operands to binary expression ('__SVMfloat8_t' and '__SVMfloat8_t')"
//@ diag: 36:12 error "cannot convert between scalar type 'int' and vector type '__SVMfloat8_t' as implicit conversion would cause truncation"
//@ diag: 37:12 error "cannot convert between vector type '__SVUint8_t' and vector type '__SVMfloat8_t' as implicit conversion would cause truncation"
//@ diag: 38:9 error "invalid argument type '__SVMfloat8_t' to unary expression"
//@ diag: 39:12 error "used type '__SVMfloat8_t' where integer is required"
//@ diag: 40:9 error "invalid argument type '__SVMfloat8_t' to unary expression"
//@ diag: 41:10 error "value of type '__SVMfloat8_t' is not contextually convertible to 'bool'"
//@ diag: 42:17 error "cannot initialize a variable of type '__SVMfloat8_t' with an lvalue of type '__SVUint8_t'"
//@ diag: 43:15 error "invalid application of 'sizeof' to sizeless type '__SVMfloat8_t'"
//@ diag: 44:12 error "invalid operands to binary expression ('m8x8' (vector of 8 '__mfp8' values) and 'm8x8')"
//@ diag: 45:12 error "invalid operands to binary expression ('m8x8' (vector of 8 '__mfp8' values) and 'm8x8')"
typedef __attribute__((neon_vector_type(8))) __mfp8 m8x8;
void vector(__SVMfloat8_t v, __SVMfloat8_t w, __SVUint8_t u, m8x8 n, m8x8 o) {
  // Allowed.
  v = v + w;
  v = v - w;
  v = v * w;
  v = v / w;
  v = -v;
  (void)(v < w);
  (void)v[0];
  n = -n;
  (void)n[0];
  // Errors.
  (void)(v & w);
  (void)(v + 1);
  (void)(v + u);
  (void)~v;
  (void)(v << 1);
  (void)!v;
  (void)(v ? v : w);
  __SVMfloat8_t x = u;
  (void)sizeof(v);
  (void)(n + o);
  (void)(n == o);
}
