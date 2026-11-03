// [ACLE] sizeless SVE types: a comma inside the parenthesized operand of a
// C-style cast is at the `(` (clang `MaybeConvertParenListExprToParenExpr`);
// a namespace-scope `auto` deduced to a sizeless type is "non-local
// variable with sizeless type"; a local SVE variable reported without
// +sve is invalid and not reported again where it is used as an operand;
// `*p && b` is a contextual conversion of a sizeless lvalue; a subscript
// of, or arithmetic on, a pointer to a sizeless type is an error (clang
// `err_subscript_incomplete_or_sizeless_type`,
// `err_typecheck_arithmetic_incomplete_or_sizeless_type`); spec/types.md §5.1.
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -ferror-limit=0
//@ strict-diags
//@ diag: 56:22 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 56:25 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 56:21 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 56:21 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 57:18 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 57:14 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 57:9 error "C-style cast from '__SVInt8_t' to 'int' is not allowed"
//@ diag: 58:25 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 58:28 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 58:21 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 58:21 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 58:21 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 56:22 warning "left operand of comma operator has no effect"
//@ diag: 57:15 warning "left operand of comma operator has no effect"
//@ diag: 58:22 warning "left operand of comma operator has no effect"
//@ diag: 58:25 warning "left operand of comma operator has no effect"
//@ diag: 55:6 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 55:6 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 60:6 error "non-local variable with sizeless type '__SVInt8_t'"
//@ diag: 61:13 error "non-local variable with sizeless type '__SVInt8_t'"
//@ diag: 63:21 error "non-local variable with sizeless type '__SVInt8_t'"
//@ diag: 69:15 error "non-local variable with sizeless type '__SVInt8_t'"
//@ diag: 74:13 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 74:10 error "value of type '__SVInt8_t' is not contextually convertible to 'bool'"
//@ diag: 74:13 error "invalid operands to binary expression ('__SVInt8_t' and 'bool')"
//@ diag: 75:12 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 75:15 error "value of type '__SVBool_t' is not contextually convertible to 'bool'"
//@ diag: 75:12 error "invalid operands to binary expression ('bool' and '__SVBool_t')"
//@ diag: 76:7 error "value of type '__SVInt8_t' is not contextually convertible to 'bool'"
//@ diag: 79:10 error "subscript of pointer to sizeless type '__SVInt8_t'"
//@ diag: 80:10 error "subscript of pointer to sizeless type '__SVBool_t'"
//@ diag: 81:10 error "subscript of pointer to sizeless type '__SVInt8_t'"
//@ diag: 82:11 error "subscript of pointer to sizeless type '__SVInt8_t'"
//@ diag: 83:12 error "arithmetic on a pointer to sizeless type '__SVInt8_t'"
//@ diag: 84:12 error "arithmetic on a pointer to sizeless type '__SVBool_t'"
//@ diag: 85:12 error "arithmetic on a pointer to sizeless type '__SVInt8_t'"
//@ diag: 86:4 error "arithmetic on a pointer to sizeless type '__SVInt8_t'"
//@ diag: 87:3 error "arithmetic on a pointer to sizeless type '__SVBool_t'"
//@ diag: 88:5 error "arithmetic on a pointer to sizeless type '__SVInt8_t'"
//@ diag: 89:12 error "arithmetic on a pointer to sizeless type '__SVInt8_t'"
__SVInt8_t *gp;
void use(__SVInt8_t);
void cast(__SVInt8_t a, __SVInt8_t b, int i) {
  (void)(__SVInt8_t)(a, b);
  (void)(int)(i, a);
  (void)(__SVInt8_t)(i, a, b);
}
auto nsa = *gp;
static auto nsb = *gp;
void local() {
  static __SVInt8_t s;
  (void)s;
  (void)-s;
  (void)&s;
  int k = 1 + s;
  use(s);
  static auto t = *gp;
  (void)t;
  (void)(1, t);
}
void ctx(__SVInt8_t *p, bool b, __SVBool_t *q) {
  (void)(*p && b);
  (void)(b || *q);
  if (*p) {}
}
void sub(__SVInt8_t *p, __SVBool_t *q, int i) {
  (void)p[0];
  (void)q[i];
  (void)0[p];
  (void)&p[1];
  (void)(p + 1);
  (void)(1 + q);
  (void)(p - 1);
  p++;
  --q;
  p += 1;
  (void)(p - p);
}
