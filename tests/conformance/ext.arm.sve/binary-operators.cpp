// [ACLE] built-in binary operators on SVE builtin types (clang
// `CheckSizelessVectorOperands`, `CheckSizelessVectorCompareOperands`):
// `+ - * /` and comparisons take two vectors of the same type, `% & | ^`
// and shifts an integer (or svbool_t, not for shifts) element type;
// different element counts, a truncating scalar, `<=>` and `&&`/`||`
// ("not contextually convertible to 'bool'" first) are errors. Without
// +sve each operand is also reported at the operator; spec/types.md §5.1.
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -ferror-limit=0
//@ strict-diags
//@ diag: 109:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 109:14 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 109:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 109:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 110:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 110:14 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 110:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 110:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 111:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 111:14 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 111:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 111:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 112:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 112:14 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 112:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 112:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 113:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 113:14 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 113:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 113:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 114:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 114:14 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 114:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 114:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 115:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 115:14 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 115:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 115:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 116:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 116:14 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 116:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 116:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 117:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 117:15 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 117:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 117:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 118:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 118:15 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 118:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 118:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 119:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 119:14 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 119:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 119:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 120:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 120:14 error "SVE vector type '__SVInt16_t' cannot be used in a target without sve"
//@ diag: 120:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 120:12 error "SVE vector type '__SVInt16_t' cannot be used in a target without sve"
//@ diag: 120:12 error "vector operands do not have the same number of elements ('__SVInt8_t' and '__SVInt16_t')"
//@ diag: 121:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 121:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 121:12 error "cannot convert between scalar type 'int' and vector type '__SVInt8_t' as implicit conversion would cause truncation"
//@ diag: 122:14 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 122:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 122:12 error "cannot convert between scalar type 'int' and vector type '__SVInt8_t' as implicit conversion would cause truncation"
//@ diag: 123:10 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 123:14 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 123:12 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 123:12 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 124:10 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 124:14 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 124:12 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 124:12 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 124:12 error "invalid operands to binary expression ('__SVBool_t' and '__SVBool_t')"
//@ diag: 125:10 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 125:14 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 125:12 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 125:12 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 126:10 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 126:14 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 126:12 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 126:12 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 126:12 error "invalid operands to binary expression ('__SVFloat32_t' and '__SVFloat32_t')"
//@ diag: 127:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 127:15 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 127:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 127:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 127:10 error "value of type '__SVInt8_t' is not contextually convertible to 'bool'"
//@ diag: 127:12 error "invalid operands to binary expression ('__SVInt8_t' and '__SVInt8_t')"
//@ diag: 128:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 128:16 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 128:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 128:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 128:12 error "three-way comparison between vectors is not supported"
//@ diag: 129:3 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 129:8 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 129:5 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 129:5 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 130:3 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 130:5 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 130:5 error "cannot convert between scalar type 'int' and vector type '__SVInt8_t' as implicit conversion would cause truncation"
//@ diag: 108:6 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 108:6 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 108:6 error "SVE vector type '__SVInt16_t' cannot be used in a target without sve"
//@ diag: 108:6 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 108:6 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 108:6 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
void bin(__SVInt8_t a, __SVInt8_t b, __SVInt16_t c, int i, __SVBool_t p, __SVBool_t q, __SVFloat32_t f) {
  (void)(a + b);
  (void)(a - b);
  (void)(a * b);
  (void)(a / b);
  (void)(a % b);
  (void)(a & b);
  (void)(a | b);
  (void)(a ^ b);
  (void)(a << b);
  (void)(a == b);
  (void)(a < b);
  (void)(a + c);
  (void)(a + i);
  (void)(i + a);
  (void)(p & q);
  (void)(p + q);
  (void)(f + f);
  (void)(f % f);
  (void)(a && b);
  (void)(a <=> b);
  a += b;
  a += i;
}
