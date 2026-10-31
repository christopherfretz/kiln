// [temp.deduct.type]/2: conflicting deductions for one parameter fail.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:12 error "no matching function for call to 'mx'"
//@ diag: 6:22 note "candidate template ignored: deduced conflicting types for parameter 'T' ('int' vs. 'double"
template <class T> T mx(T a, T b) { return a < b ? b : a; }
double d = mx(1, 2.0);
