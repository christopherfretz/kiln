// C++17 [temp.arg.nontype]/2: a pointer or reference argument may be
// any converted constant expression, but not the address of a
// subobject (a base, a member, an array element other than the first,
// to which the array decays, or one past the end). clang 22's error
// (GCC 16.1 rejects the same arguments); before C++17 the form is
// checked (temp.arg.nontype/address-form-before-cxx17.cpp), from C++20
// subobjects are allowed. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ args: -std=c++17
//@ diag: 47:3 error "non-type template argument refers to subobject '&a[2]'"
//@ diag: 48:3 error "non-type template argument refers to subobject '&a[1]'"
//@ diag: 49:3 error "non-type template argument refers to subobject '&s.m'"
//@ diag: 50:3 error "non-type template argument refers to subobject '&b"
//@ diag: 51:3 error "non-type template argument refers to subobject 's.m'"
//@ diag: 52:3 error "non-type template argument refers to subobject 'a[1]'"
//@ diag: 53:3 error "conversion from 'B *' to 'const A *' is not allowed in a converted constant expression"
//@ diag: 29:20 note "template parameter is declared here"
//@ diag: 54:4 error "conversion from 'B' to 'const A &' is not allowed in a converted constant expression"
//@ diag: 30:20 note "template parameter is declared here"
//@ diag: 55:3 error "non-type template argument refers to subobject '&bb."

template <int *p> struct X {};
template <int &r> struct R {};
template <const void *p> struct V {};
struct A { int i; };
struct B : A {};
template <const A *p> struct C {};
template <const A &r> struct CR {};
int a[3];
int b;
B bb;
struct S { int m; static int s; } s;
int S::s;
constexpr int *id(int *p) { return p; }

X<&b> ok1;
X<a> ok2;
X<&a[0]> ok3;
X<id(&b)> ok4;
X<&s.s> ok5;
R<*&b> ok6;
R<S::s> ok7;
X<nullptr> ok8;

X<&a[2]> e1;
X<a + 1> e2;
X<&s.m> e3;
X<&b + 1> e4;
R<s.m> e5;
R<a[1]> e6;
C<&bb> e7;
CR<bb> e8;
X<&bb.i> e9;
