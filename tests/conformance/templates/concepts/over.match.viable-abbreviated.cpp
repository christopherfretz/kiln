//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:12 error "no matching function for call to 'k'"
//@ diag: 9:6 note "candidate template ignored: constraints not satisfied [with a:auto = double]"
//@ diag: 9:8 note "because 'double' does not satisfy 'Int'"
//@ diag: 8:34 note "because '__is_same(double, int)' evaluated to false"
// [dcl.fct]/22: an unsatisfied constrained invented parameter.
template <class T> concept Int = __is_same(T, int);
void k(Int auto a);
void m() { k(2.0); }
