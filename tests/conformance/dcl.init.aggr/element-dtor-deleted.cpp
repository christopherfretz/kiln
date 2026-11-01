// [dcl.init.aggr]/8, [class.dtor]/15: a deleted element destructor
// makes the aggregate initialization ill-formed, and the first one ends
// the checking of the list (WP-227).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:8 error "attempt to use a deleted function"
//@ diag: 16:12 note "'~X' has been explicitly marked deleted here"
//@ diag: 19:9 error "attempt to use a deleted function"
//@ diag: 16:12 note "'~X' has been explicitly marked deleted here"
//@ diag: 20:12 error "attempt to use a deleted function"
//@ diag: 16:12 note "'~X' has been explicitly marked deleted here"
//@ diag: 21:3 error "attempt to use a deleted function"
//@ diag: 16:12 note "'~X' has been explicitly marked deleted here"
//@ diag: 23:9 error "attempt to use a deleted function"
//@ diag: 16:12 note "'~X' has been explicitly marked deleted here"
struct X { ~X() = delete; };
struct Y { X x; int k; ~Y(); };
Y y = {};
Y y2 = {1, 2};
X xa[2] = {};
X xb[2];
union U { int i; X x; ~U(); };
U u = {1};
