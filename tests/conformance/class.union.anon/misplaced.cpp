// [class.union.anon]/1-2: placement and storage of anonymous unions and structs
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:1 error "anonymous unions at namespace or global scope must be declared 'static'"
//@ diag: 12:1 error "anonymous structs and classes must be class members"
//@ diag: 13:61 error "duplicate member 'd'"
//@ diag: 13:51 note "previous declaration is here"
//@ diag: 14:31 error "member of anonymous union redeclares 'z'"
//@ diag: 14:16 note "previous declaration is here"
//@ diag: 14:37 error "anonymous structs and classes must be class members"
union { int a; };
struct { int b; };
struct T { struct { int c; }; union { int c2; int d; }; int d; };
void f() { int z; union { int z; }; struct { int w; }; }
