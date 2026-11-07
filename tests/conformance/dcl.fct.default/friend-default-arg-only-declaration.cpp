// [dcl.fct.default]/4: a friend declaration that specifies a default
// argument shall be a definition and the only declaration of the function
// or function template in the translation unit. Reported on the friend
// when it redeclares, or on a later redeclaration of such a friend.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 27:24 error "friend declaration specifying a default argument must be the only declaration"
//@ diag: 26:6 note "previous declaration is here"
//@ diag: 28:24 error "friend declaration specifying a default argument must be a definition"
//@ diag: 30:6 error "friend declaration specifying a default argument must be the only declaration"
//@ diag: 29:24 note "previous declaration is here"
//@ diag: 32:24 error "friend declaration specifying a default argument must be the only declaration"
//@ diag: 31:24 note "previous declaration is here"
//@ diag: 33:43 error "friend declaration specifying a default argument must be a definition"
//@ diag: 36:6 error "friend declaration specifying a default argument must be the only declaration"
//@ diag: 34:44 note "previous declaration is here"
//@ diag: 37:43 error "friend declaration specifying a default argument must be a definition"
//@ diag: 39:25 error "friend declaration specifying a default argument must be the only declaration"
//@ diag: 38:43 note "previous declaration is here"
//@ diag: 40:54 error "friend declaration specifying a default argument must be the only declaration"
//@ diag: 40:20 note "previous declaration is here"
//@ diag: 41:50 error "friend declaration specifying a default argument must be the only declaration"
//@ diag: 41:24 note "previous declaration is here"
void f(int);
struct S { friend void f(int = 3) {} };
struct T { friend void g(int = 3); };
struct U { friend void h(int = 3) {} };
void h(int);
struct V { friend void k(int = 3) {} };
struct W { friend void k(int); };
template <class X> struct Z { friend void q(X = 0); };
template <class X> struct Z2 { friend void q2(X = 0) {} };
Z2<int> z2;
void q2(int);
struct A { template <class X> friend void tf(X, int = 0); };
struct B { template <class X> friend void tf2(X, int = 0) {} };
template <class X> void tf2(X, int);
namespace N { void r(int); struct C { friend void N::r(int = 0); }; }
struct D { friend void s(int = 1) {} friend void s(int); };
void ok(int = 2);
struct E { friend void ok(int); };
template <class X> struct F { friend void fine(F, int = 0) {} };
F<int> fi;
F<long> fl;
void use() { fine(fi); fine(fl); ok(); }
