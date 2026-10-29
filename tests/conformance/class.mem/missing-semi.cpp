// [class.pre]/1, [dcl.enum]/1: a class or enumeration definition is followed by `;` or a declarator
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:20 error "expected ';' after struct"
//@ diag: 11:11 error "expected ';' after class"
//@ diag: 12:19 error "expected ';' after union"
//@ diag: 13:14 error "expected ';' after enum"
//@ diag: 15:25 error "expected ';' after enum"
struct S { int a; }
int x;
class C {}
union U { int i; }
enum E { e0 }
int y;
struct T { enum F { f0 } int z; };
int after = sizeof(S) + sizeof(T);
