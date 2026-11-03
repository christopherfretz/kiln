// [ACLE] SVE builtin types with scalar operands and unary operators (clang
// `CheckSizelessVectorOperands`, `CheckVectorOperands` splat rules): a
// scalar operand is converted to the element type when that does not
// truncate, a comparison yields the signed vector of the same element
// width (svbool_t for svbool_t), `+`/`-` apply to any SVE vector and `~`
// to an integer one, while `!` and `++` are errors; spec/types.md §5.1.
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -ferror-limit=0
//@ strict-diags
//@ diag: 113:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 113:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 114:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 114:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 114:12 error "cannot convert between scalar type 'int' and vector type '__SVInt8_t' as implicit conversion would cause truncation"
//@ diag: 115:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 115:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 116:10 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 116:12 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 117:10 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 117:12 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 118:10 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 118:12 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 118:12 error "cannot convert between scalar type 'double' and vector type '__SVFloat32_t' as implicit conversion would cause truncation"
//@ diag: 119:10 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 119:12 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 119:12 error "cannot convert between scalar type 'int' and vector type '__SVFloat32_t' as implicit conversion would cause truncation"
//@ diag: 120:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 120:14 error "SVE vector type '__SVUint8_t' cannot be used in a target without sve"
//@ diag: 120:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 120:12 error "SVE vector type '__SVUint8_t' cannot be used in a target without sve"
//@ diag: 120:12 error "cannot convert between vector type '__SVUint8_t' and vector type '__SVInt8_t' as implicit conversion would cause truncation"
//@ diag: 121:10 error "SVE vector type '__SVInt32_t' cannot be used in a target without sve"
//@ diag: 121:14 error "SVE vector type '__SVInt32_t' cannot be used in a target without sve"
//@ diag: 122:10 error "SVE vector type '__SVInt32_t' cannot be used in a target without sve"
//@ diag: 122:14 error "SVE vector type '__SVInt32_t' cannot be used in a target without sve"
//@ diag: 122:14 error "cannot convert between scalar type 'long' and vector type '__SVInt32_t' as implicit conversion would cause truncation"
//@ diag: 123:10 error "SVE vector type '__SVInt32_t' cannot be used in a target without sve"
//@ diag: 123:14 error "SVE vector type '__SVInt32_t' cannot be used in a target without sve"
//@ diag: 124:15 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 124:13 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 125:10 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 125:15 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 125:12 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 125:12 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 125:12 warning "self-comparison always evaluates to true"
//@ diag: 126:10 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 126:15 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 126:12 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 126:12 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 126:12 warning "self-comparison always evaluates to true"
//@ diag: 127:10 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 127:14 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 127:12 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 127:12 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 127:12 warning "self-comparison always evaluates to false"
//@ diag: 128:10 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 128:15 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 128:12 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 128:12 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 128:12 error "invalid operands to binary expression ('__SVBool_t' and '__SVBool_t')"
//@ diag: 129:10 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 129:15 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 129:12 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 129:12 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 129:12 error "used type '__SVFloat32_t' where integer is required"
//@ diag: 130:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 130:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 131:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 131:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 132:10 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 132:14 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 132:12 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 132:12 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 132:12 error "invalid operands to binary expression ('__SVBool_t' and '__SVBool_t')"
//@ diag: 133:10 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 133:14 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 133:12 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 133:12 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 134:10 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 134:14 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 134:12 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 134:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 134:12 error "cannot convert between vector type '__SVInt8_t' and vector type '__SVBool_t' as implicit conversion would cause truncation"
//@ diag: 135:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 135:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 135:10 error "value of type '__SVInt8_t' is not contextually convertible to 'bool'"
//@ diag: 135:12 error "invalid operands to binary expression ('__SVInt8_t' and 'int')"
//@ diag: 136:15 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 136:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 136:15 error "value of type '__SVInt8_t' is not contextually convertible to 'bool'"
//@ diag: 136:12 error "invalid operands to binary expression ('int' and '__SVInt8_t')"
//@ diag: 138:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 139:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 140:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 140:9 error "invalid argument type '__SVInt8_t' to unary expression"
//@ diag: 141:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 142:10 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 143:10 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 144:10 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 144:9 error "invalid argument type '__SVFloat32_t' to unary expression"
//@ diag: 145:5 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 145:3 error "cannot increment value of type '__SVInt8_t'"
//@ diag: 146:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 146:12 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 147:10 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 147:12 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 112:6 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 112:6 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 112:6 error "SVE vector type '__SVFloat32_t' cannot be used in a target without sve"
//@ diag: 112:6 error "SVE vector type '__SVUint8_t' cannot be used in a target without sve"
//@ diag: 112:6 error "SVE vector type '__SVInt32_t' cannot be used in a target without sve"
void un(__SVInt8_t a, __SVBool_t p, __SVFloat32_t f, __SVUint8_t u, int i, char c, double d, signed char sc, __SVInt32_t a32, long l) {
  (void)(a + 1);
  (void)(a + 1000);
  (void)(a + sc);
  (void)(f + 1.0);
  (void)(f + 1.0f);
  (void)(f + d);
  (void)(f + i);
  (void)(a + u);
  (void)(a32 + i);
  (void)(a32 + l);
  (void)(a32 + 1L);
  (void)(sc + a);
  (void)(p == p);
  (void)(f == f);
  (void)(p < p);
  (void)(p << p);
  (void)(f << f);
  (void)(a << 1);
  (void)(a << i);
  (void)(p * p);
  (void)(p ^ p);
  (void)(p ^ a);
  (void)(a || i);
  (void)(i && a);
  
  (void)-a;
  (void)~a;
  (void)!a;
  (void)+a;
  (void)-p;
  (void)~p;
  (void)~f;
  ++a;
  (void)(a == 1);
  (void)(f > 1.0f);
}
