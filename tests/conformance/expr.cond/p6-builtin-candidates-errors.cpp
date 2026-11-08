// [expr.cond]/6: when the built-in candidates for the arms are
// ambiguous, or none is viable, the conditional is ill-formed. WP-799.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:43 error "conditional expression is ambiguous; 'two' and 'other' can be converted to several common types"
//@ diag: 18:28 error "incompatible operand types ('none' and 'int *')"
struct two {
  operator int *() const;
  operator double *() const;
};
struct other {
  operator int *() const;
  operator double *() const;
};
struct none {};
bool b;
void f(two x, other y, long l) { (void)(b ? x : y); }
void g(none n, int *p) { b ? n : p; }
