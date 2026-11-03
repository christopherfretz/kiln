// [temp.arg.nontype]/2, [expr.const]/12: a converted constant expression
// admits a pointer (or pointer-to-member) conversion only from
// std::nullptr_t, and a boolean conversion only from an integral or
// unscoped enumeration type. So `&s` does not convert to `const void *`
// or to a base pointer, and a pointer does not convert to `bool`
// (clang `CheckConvertedConstantConversions`). An explicitly-specified
// argument of a function template fails deduction instead.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 42:12 error "no matching function for call to 'f'"
//@ diag: 34:31 note "candidate template ignored: invalid explicitly-specified argument for template parameter 'P'"
//@ diag: 43:3 error "conversion from 'S *' to 'const void *' is not allowed in a converted constant expression"
//@ diag: 44:3 error "conversion from 'D *' to 'const void *' is not allowed in a converted constant expression"
//@ diag: 45:3 error "conversion from 'D *' to 'const B *' is not allowed in a converted constant expression"
//@ diag: 46:3 error "conversion from 'D *' to 'bool' is not allowed in a converted constant expression"
//@ diag: 47:3 error "conversion from 'int D::*' to 'bool' is not allowed in a converted constant expression"
//@ diag: 48:3 error "conversion from 'int B::*' to 'int D::*' is not allowed in a converted constant expression"
//@ diag: 49:12 error "conversion from 'D *' to 'const void *' is not allowed in a converted constant expression"
//@ diag: 50:3 error "conversion from 'int[3]' to 'const void *' is not allowed in a converted constant expression"
//@ diag: 51:3 error "conversion from 'int (*)[3]' to 'const void *' is not allowed in a converted constant expression"
//@ diag: 52:3 error "conversion from 'const char[4]' to 'const void *' is not allowed in a converted constant expression"
//@ diag: 53:3 error "conversion from 'int' to 'const void *' is not allowed in a converted constant expression"
//@ diag: 54:4 error "conversion from 'int' to 'int *' is not allowed in a converted constant expression"
//@ diag: 55:3 error "conversion from 'C' to 'const void *' is not allowed in a converted constant expression"
//@ requires: clang<22
// clang 19 spelling; pointer-conversion-not-allowed-clang22.cpp has
// clang 22's (the parameter note) (WP-407).
struct S {};
S s;
struct B { int x; };
struct D : B { int y; };
D d;
int arr[3];
template <const void *P> void f() {}
template <const void *P> struct X {};
template <const B *P> struct Y {};
template <bool V> struct Z {};
template <int D::*Mp> struct M {};
template <int *P> struct IP {};
template <const void *P> constexpr int vt = 0;
struct C { constexpr operator S *() const { return &s; } };
void t() { f<&s>(); }
X<&s> x;
X<&d> xd;
Y<&d> y;
Z<&d> z;
Z<&D::y> zm;
M<&B::x> m;
int k = vt<&d>;
X<arr> xa;
X<&arr> xb;
X<"lit"> xl;
X<0> x0;
IP<0> ip0;
X<C{}> xc;
