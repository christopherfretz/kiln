// [ACLE] SVE builtin types without +sve: clang `checkTypeSupport`
// (err_sve_vector_in_non_sve_target) for each operand of a built-in binary
// operator at the operator, for a member function named without a call
// at its name (before "must be called"), and for a lambda call operator
// whose parameter or (deduced) return type is SVE at its `[`;
// spec/types.md §5.1.
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -ferror-limit=0
//@ strict-diags
//@ diag: 46:3 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 46:7 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 46:5 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 46:5 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 45:6 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 45:6 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 49:7 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 49:7 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 53:10 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 53:8 warning "left operand of comma operator has no effect"
//@ diag: 56:8 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 56:6 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 56:6 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 55:6 error "SVE vector type '__SVBool_t' cannot be used in a target without sve"
//@ diag: 59:15 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 59:15 error "invalid application of 'sizeof' to sizeless type '__SVInt8_t'"
//@ diag: 60:16 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 68:5 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 68:5 error "reference to non-static member function must be called"
//@ diag: 69:5 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 70:11 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 70:11 error "reference to non-static member function must be called"
//@ diag: 71:8 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 69:5 warning "expression result unused"
//@ diag: 71:5 warning "expression result unused"
//@ diag: 75:18 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 76:19 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 77:16 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 78:18 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 79:60 note "in instantiation of function template specialization"
//@ diag: 79:60 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 62:41 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 63:19 note "in instantiation of function template specialization"
//@ diag: 62:41 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
__SVInt8_t *gp;
void assign(__SVInt8_t x, __SVInt8_t y) {
  x = y;
}
void deref() {
  *gp = *gp;
}
void comma(__SVInt8_t *p, int i) {
  (void)i;
  i = (*p, 0);
}
void compound(__SVBool_t *p, __SVBool_t q) {
  *p = q;
}
void unevaluated() {
  (void)sizeof(*gp = *gp);
  decltype(*gp = *gp) r = *gp;
}
template <class T> void tmpl(T *p) { *p = *p; }
void use_tmpl() { tmpl(gp); }
struct C {
  void mem(__SVInt8_t);
  static void sm(__SVInt8_t);
  void mm() {
    mem;
    sm;
    this->mem;
    C::sm;
  }
};
void lambdas(__SVInt8_t *p) {
  auto deduced = [p] { return *p; };
  auto trailing = [](__SVInt8_t *q) -> __SVInt8_t { return *q; };
  auto param = [](__SVInt8_t q) {};
  auto generic = [](auto *q) { return *q; };
  (void)deduced; (void)trailing; (void)param; (void)generic(p);
}
