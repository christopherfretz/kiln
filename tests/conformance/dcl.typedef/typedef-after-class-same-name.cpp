// [dcl.typedef]/6, [basic.scope.scope]/4: a typedef of a class's name in
// the class's scope must name that class; `typedef long t1;` after
// `struct t1` is clang's "typedef redefinition with different types".
// Found auditing WP-1427 (C++ only; C keeps tags apart, C17 6.2.3).
//@ expect: compile-fail
//@ strict-diags
//@ xfail(kiln): WP-1427 follow-up, a typedef declared after a class of the same name is not checked against it
//@ diag: 11:14 error "typedef redefinition with different types ('long' vs 't1')"
//@ diag: 10:8 note "previous definition is here"
struct t1 { int a; };
typedef long t1;
