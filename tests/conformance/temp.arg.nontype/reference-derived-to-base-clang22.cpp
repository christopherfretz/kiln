// [temp.arg.nontype]/2, [expr.const]/12, [over.ics.ref]/1: a template
// argument for a reference parameter is a converted constant expression,
// whose implicit conversion sequence may not contain a derived-to-base
// conversion. Binding `const B &` to an object of a class derived from
// `B` is one (also after a user-defined conversion), so it is not
// allowed; binding to an lvalue of type `B` that designates the base
// subobject is an identity binding, and is (WP-561; clang
// `CheckConvertedConstantConversions`). The pointer case is
// pointer-conversion-not-allowed.cpp.
// clang 22 spelling (the parameter note); clang 19's is in
// reference-derived-to-base.cpp.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 57:3 error "conversion from 'const D' to 'const B &' is not allowed in a converted constant expression"
//@ diag: 40:20 note "template parameter is declared here"
//@ diag: 58:3 error "conversion from 'const D' to 'const B &' is not allowed in a converted constant expression"
//@ diag: 40:20 note "template parameter is declared here"
//@ diag: 59:4 error "conversion from 'const DD' to 'const D &' is not allowed in a converted constant expression"
//@ diag: 41:20 note "template parameter is declared here"
//@ diag: 60:3 error "conversion from 'D' to 'B &' is not allowed in a converted constant expression"
//@ diag: 42:14 note "template parameter is declared here"
//@ diag: 61:7 error "conversion from 'const D' to 'const B &' is not allowed in a converted constant expression"
//@ diag: 43:29 note "template parameter is declared here"
//@ diag: 62:3 error "conversion from 'const E' to 'const B &' is not allowed in a converted constant expression"
//@ diag: 40:20 note "template parameter is declared here"
//@ diag: 63:9 error "no matching function for call to 'f'"
//@ diag: 44:37 note "candidate template ignored: invalid explicitly-specified argument for template parameter 'R'"
//@ diag: 64:31 error "conversion from 'const D' to 'const B &' is not allowed in a converted constant expression"
//@ diag: 40:20 note "template parameter is declared here"
//@ diag: 66:23 error "conversion from 'const D' to 'const B &' is not allowed in a converted constant expression"
//@ diag: 65:20 note "template parameter is declared here"
//@ requires: clang>=22
struct B { int x; };
struct D : B { int y; };
struct DD : D {};
constexpr D d{{7}, 2};
constexpr DD dd{};
constexpr D arr[2]{};
D md;
template <const B &R> struct S { static constexpr int v = R.x; };
template <const D &R> struct SD {};
template <B &R> struct M {};
template <class T, const T &R> struct ST {};
template <const B &R> constexpr int f() { return 1; }
template <B V> struct C {};
struct E { constexpr operator const D &() const { return d; } };
constexpr E e{};
// Valid: an lvalue of the base type designating the base subobject
// (identity binding), a qualification conversion, a class-type
// parameter copy-initialized from a derived object.
static_assert(S<static_cast<const B &>(d)>::v == 7);
template <const volatile D &R> struct SV {};
SV<d> sv;
C<d> c;
SD<e> sde;
// Invalid: the binding is a derived-to-base conversion.
S<d> s;
S<(d)> sp;
SD<dd> sdd;
M<md> m;
ST<B, d> st;
S<e> se;
int i = f<d>();
template <int N> struct W { S<arr[N]> w; };
template <const B &R> struct S2 {};
template <> struct S2<d> {};
