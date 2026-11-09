// C11 6.5.1.1p2 (_Generic in C++, a clang extension): clang 22 accepts an
// incomplete association type as a C2y extension (N3409), warned under
// -pedantic [-Wc2y-extensions] next to the C11 extension warning on the
// keyword [-Wc11-extensions]; the type operand form is a C2y extension
// too (N3260). Clang 19: generic-selection-incomplete-19.cpp.
//@ requires: clang>=22
//@ expect: compile-ok
//@ args: -pedantic
//@ strict-diags
//@ diag: 20:9 warning "'_Generic' is a C11 extension"
//@ diag: 20:21 warning "incomplete type 'Inc' in a '_Generic' association is a C2y extension"
//@ diag: 21:10 warning "'_Generic' is a C11 extension"
//@ diag: 21:22 warning "incomplete type 'Inc[]' in a '_Generic' association is a C2y extension"
//@ diag: 22:10 warning "'_Generic' is a C11 extension"
//@ diag: 22:22 warning "incomplete type 'void' in a '_Generic' association is a C2y extension"
//@ diag: 23:9 warning "'_Generic' is a C11 extension"
//@ diag: 23:18 warning "passing a type argument as the first operand to '_Generic' is a C2y extension"
struct Inc;
int x;
int d = _Generic(x, Inc: 1, default: 2);
int d2 = _Generic(x, Inc[]: 1, default: 2);
int d3 = _Generic(x, void: 1, default: 2);
int e = _Generic(Inc, Inc: 1, default: 2);
