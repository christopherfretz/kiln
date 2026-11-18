// [expr.context]/2 with [conv.lval]/1, from C++11 (a recorded deviation,
// docs/status/deviations.md): a discarded-value expression that is a
// volatile glvalue of an incomplete class type and an id-expression (a
// variable, or a reference variable) undergoes the lvalue-to-rvalue
// conversion, which needs a complete type, also as the operand of a cast
// to `void` ([expr.static.cast]/6). clang 22 only warns "expression result
// unused" (and not for the casts); GCC 16.1 warns that the object "will
// not be accessed". Found running GCC's C++ testsuite (WP-1533, WP-1692).
//@ xfail(clang): clang 19 and 22 do not require a complete type for a discarded volatile glvalue
//@ expect: compile-fail
//@ std: c++11
//@ strict-diags
//@ diag: 22:3 error "incomplete type 'volatile I' where a complete type is required"
//@ diag: 23:3 error "incomplete type 'volatile I' where a complete type is required"
//@ diag: 24:9 error "incomplete type 'volatile I' where a complete type is required"
//@ diag: 25:9 error "incomplete type 'volatile I' where a complete type is required"
struct I;
extern volatile I v;
extern volatile I &r;
extern I w;
void t() {
  v;
  r;
  (void)v;
  (void)r;
  (void)w; // not volatile: not converted
}
