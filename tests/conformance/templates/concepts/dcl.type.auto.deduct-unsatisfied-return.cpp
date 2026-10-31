//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:1 error "deduced type 'double' does not satisfy 'Int'"
//@ diag: 6:34 note "because '__is_same(double, int)' evaluated to false"
// [dcl.spec.auto.general]/9: a constrained placeholder return type.
template <class T> concept Int = __is_same(T, int);
Int auto f() { return 1.0; }
