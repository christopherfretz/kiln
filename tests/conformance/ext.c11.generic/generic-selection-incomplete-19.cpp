// C11 6.5.1.1p2 (_Generic in C++, a clang extension): clang 19 rejects an
// incomplete association type (void, a forward-declared class, an array
// of unknown bound) when the operand is an expression; the type operand
// form accepts it (clang 19 CreateGenericSelectionExpr). Clang 22 makes it
// a C2y extension: generic-selection-incomplete-c2y.cpp.
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:21 error "type 'Inc' in generic association incomplete"
//@ diag: 15:22 error "type 'Inc[]' in generic association incomplete"
//@ diag: 16:22 error "type 'void' in generic association incomplete"
struct Inc;
int x;
int d = _Generic(x, Inc: 1, default: 2);
int d2 = _Generic(x, Inc[]: 1, default: 2);
int d3 = _Generic(x, void: 1, default: 2);
int e = _Generic(Inc, Inc: 1, default: 2);
