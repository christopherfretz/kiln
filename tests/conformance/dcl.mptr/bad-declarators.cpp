// [dcl.mptr]/1: the nested-name-specifier of a pointer to member names a
// class; [dcl.mptr]/3: no pointer to a member of reference type or of type
// `cv void`. [basic.lookup.qual]/1: a name before `::` that is not a class,
// namespace or enumeration.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:5 error "'I' (aka 'int') is not a class, namespace, or enumeration"
//@ diag: 24:5 error "'p2' does not point into a class"
//@ diag: 25:5 error "'p3' does not point into a class"
//@ diag: 26:6 error "'p4' declared as a member pointer to a reference of type 'int &'"
//@ diag: 27:6 error "'p5' declared as a member pointer to void"
//@ diag: 28:7 error "'p6' declared as a member pointer to a reference of type 'int &'"
//@ diag: 29:5 error "use of undeclared identifier 'nope'"
//@ diag: 30:21 error "'type name' does not point into a class"
//@ diag: 31:22 error "'type name' declared as a member pointer to a reference of type 'int &'"
//@ diag: 32:12 error "'type name' does not point into a class"
//@ diag: 33:13 error "'type name' declared as a member pointer to void"
//@ diag: 34:16 error "cannot form pointer to 'decltype(auto)'"
struct C { int m; };
typedef int I;
enum E { e1 };
namespace N { int x; }
int I::*p1;
int E::*p2;
int N::*p3;
int &C::*p4;
void C::*p5;
int &(C::*p6);
int nope::*p7;
int s1 = sizeof(int E::*);
int s2 = sizeof(int &C::*);
void g(int N::*);
void h(void C::*);
decltype(auto) C::*p8 = &C::m;
int C::*&r = p1;
