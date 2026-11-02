// [dcl.mptr]/1: the nested-name-specifier of a pointer to member names a
// class; [dcl.mptr]/3: no pointer to a member of reference type or of type
// `cv void`. [basic.lookup.qual]/1: a name before `::` that is not a class,
// namespace or enumeration.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:5 error "'I' (aka 'int') is not a class, namespace, or enumeration"
//@ diag: 26:5 error "'p2' does not point into a class"
//@ diag: 27:5 error "'p3' does not point into a class"
//@ diag: 28:6 error "'p4' declared as a member pointer to a reference of type 'int &'"
//@ diag: 29:6 error "'p5' declared as a member pointer to void"
//@ diag: 30:7 error "'p6' declared as a member pointer to a reference of type 'int &'"
//@ diag: 31:5 error "use of undeclared identifier 'nope'"
//@ diag: 32:21 error "does not point into a class"
//@ diag: 33:22 error "'type name' declared as a member pointer to a reference of type 'int &'"
//@ diag: 34:12 error "does not point into a class"
//@ diag: 35:13 error "'type name' declared as a member pointer to void"
//@ diag: 36:16 error "cannot form pointer to 'decltype(auto)'"
// clang 19 and 22 word these differently: the substrings above are common
// to both, and kiln follows its -fclang-compat level (WP-377).
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
