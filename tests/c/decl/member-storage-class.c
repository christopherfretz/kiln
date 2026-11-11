// C23 6.7.2.1p1 (structure and union specifiers): a member declaration
// takes a specifier-qualifier-list, which has no storage-class specifier
// (clang's "type name does not allow storage class to be specified"); the
// specifier is dropped and the member declared as written.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:12 error "type name does not allow storage class to be specified"
//@ diag: 13:12 error "type name does not allow storage class to be specified"
//@ diag: 14:12 error "type name does not allow storage class to be specified"
//@ diag: 15:12 error "type name does not allow storage class to be specified"
//@ diag: 16:12 error "type name does not allow storage class to be specified"
struct A { static int x; int y; };
struct B { extern int x; int y; };
struct C { register int x; int y; };
struct D { typedef int t; int y; };
struct E { _Thread_local int x; int y; };
int g(struct A *a) { return a->x + a->y; }
