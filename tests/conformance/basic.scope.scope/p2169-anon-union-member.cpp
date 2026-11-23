// [basic.scope.scope]/5 (C++26, P2169R4): a non-static data member is
// name-independent unless it is a member of an anonymous union (and, for
// the anonymous struct extension, of an anonymous struct), so a `_`
// member of one conflicts with an earlier `_` of the scope its names are
// injected into, and with another member `_` of the same union; a later
// name-independent declaration does not conflict with it. clang 22 and
// GCC 16.1 accept every form (clang counts every data member as a
// placeholder). The ambiguity wording is in
// p2169-anon-union-member-ambiguous.cpp.
//@ skip(clang): clang accepts a `_` member of an anonymous union that conflicts
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:31 error "member of anonymous union redeclares '_'"
//@ diag: 28:16 note "previous declaration is here"
//@ diag: 29:43 error "member of anonymous union redeclares '_'"
//@ diag: 29:24 note "previous declaration is here"
//@ diag: 30:31 error "member of anonymous union redeclares '_'"
//@ diag: 30:24 note "previous declaration is here"
//@ diag: 31:32 error "member of anonymous struct redeclares '_'"
//@ diag: 31:25 note "previous declaration is here"
//@ diag: 34:32 error "member of anonymous union redeclares '_'"
//@ diag: 34:24 note "previous declaration is here"
//@ diag: 35:35 error "member of anonymous union redeclares '_'"
//@ diag: 35:16 note "previous declaration is here"
//@ diag: 38:48 error "member of anonymous union redeclares '_'"
//@ diag: 38:35 note "previous declaration is here"
struct S { int _; union { int _; float f; }; };
struct T { union { int _; }; union { long _; }; };
struct V { union { int _; int _; }; };
struct W { struct { int _; int _; }; };
struct X { int _; int _; union { int a; }; };
struct Y { union { int _; }; int _; };
void f() { union { int _; long _; }; }
void g() { int _ = 1; union { int _; }; }
void h() { union { int _; }; int _ = 1; }
void i() { static union { int _; }; int _ = 2; }
template <class U> struct Z { int _; union { U _; }; };
template <class U> struct Q { union { U _; }; U _; };
Q<int> q;
union { int _; } named;
int _;
