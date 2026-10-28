// [class.union.anon]/1: the members of an anonymous union are declared in the enclosing scope
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:20 error "member of anonymous union redeclares 'x'"
//@ diag: 8:5 note "previous declaration is here"
//@ diag: 10:31 error "member of anonymous union redeclares 'a'"
//@ diag: 10:16 note "previous declaration is here"
int x;
static union { int x; long y; };
struct S { int a; union { int a; }; };
