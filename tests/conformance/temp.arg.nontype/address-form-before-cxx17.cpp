// C++98 to C++14 [temp.arg.nontype]/1: a pointer argument is written
// `&`id-expression (or names an array or function), a reference
// argument is an id-expression: the address of an array element, a
// member, a base or anything computed was not an argument. clang 22's
// errors (GCC 16.1 rejects the same arguments); C++17 allows any
// constant expression except a subobject
// (temp.arg.nontype/subobject-address-cxx17.cpp), C++20 anything.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ args: -std=c++14
//@ diag: 71:4 error "non-type template argument does not refer to any declaration"
//@ diag: 38:16 note "template parameter is declared here"
//@ diag: 72:4 error "non-type template argument does not refer to any declaration"
//@ diag: 38:16 note "template parameter is declared here"
//@ diag: 73:4 error "non-type template argument does not refer to any declaration"
//@ diag: 38:16 note "template parameter is declared here"
//@ diag: 74:4 error "non-type template argument does not refer to any declaration"
//@ diag: 38:16 note "template parameter is declared here"
//@ diag: 75:4 error "non-type template argument does not refer to any declaration"
//@ diag: 38:16 note "template parameter is declared here"
//@ diag: 76:3 error "non-type template argument for template parameter of pointer type 'int *' must have its address taken"
//@ diag: 38:16 note "template parameter is declared here"
//@ diag: 77:3 error "non-type template argument does not refer to any declaration"
//@ diag: 38:16 note "template parameter is declared here"
//@ diag: 78:3 error "non-type template argument does not refer to any declaration"
//@ diag: 38:16 note "template parameter is declared here"
//@ diag: 79:3 error "non-type template argument does not refer to any declaration"
//@ diag: 39:16 note "template parameter is declared here"
//@ diag: 80:3 error "non-type template argument does not refer to any declaration"
//@ diag: 39:16 note "template parameter is declared here"
//@ diag: 81:3 error "non-type template argument does not refer to any declaration"
//@ diag: 39:16 note "template parameter is declared here"
//@ diag: 82:3 error "non-type template argument does not refer to any declaration"
//@ diag: 44:20 note "template parameter is declared here"

template <int *p> struct X {};
template <int &r> struct R {};
template <void (*f)()> struct F {};
template <void (&f)()> struct FR {};
struct A {};
struct B : A {};
template <const A *p> struct C {};
int a[3];
int b;
B bb;
struct S { int m; static int s; } s;
int S::s;
constexpr int *q = &b;
constexpr int *id(int *p) { return p; }
void g();
void g(int);

X<&b> ok1;
X<(&b)> ok2;
X<a> ok3;
X<&S::s> ok4;
X<&::b> ok5;
X<nullptr> ok6;
X<(int *)0> ok7;
R<b> ok8;
R<(b)> ok9;
R<S::s> ok10;
F<g> ok11;
F<&g> ok12;
FR<g> ok13;
template <int *p> struct Y { X<p> y; };
Y<&b> ok14;

X<&(b)> e1;
X<&a[0]> e2;
X<&a[2]> e3;
X<&s.m> e4;
X<&s.s> e5;
X<q> e6;
X<id(&b)> e7;
X<a + 1> e8;
R<s.m> e9;
R<a[1]> e10;
R<*&b> e11;
C<static_cast<const A *>(&bb)> e12;
