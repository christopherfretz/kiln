//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:1 error "deduced type 'double' does not satisfy 'Int'"
//@ diag: 7:34 note "because '__is_same(double, int)' evaluated to false"
// [dcl.type.auto.deduct]/2: the deduced type does not satisfy the
// placeholder's type-constraint.
template <class T> concept Int = __is_same(T, int);
Int auto x = 1.0;
