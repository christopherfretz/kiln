// [expr.reinterpret.cast]/2,10, [expr.cast]/4: a reinterpret_cast between
// pointer-to-member types casts away no constness; a pointer to data member
// and a pointer to member function never convert to each other; the
// conversion is not a core constant expression ([expr.const]/10.15).
// Found running GCC's C++ testsuite (WP-1467).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:9 error "reinterpret_cast from 'const long A::*' to 'aip' (aka 'int A::*') casts away qualifiers"
//@ diag: 20:14 error "reinterpret_cast from 'const int A::*' to 'long A::*' casts away qualifiers"
//@ diag: 22:8 error "C-style cast from 'int A::*' to 'F0' (aka 'int (A::*)()') is not allowed"
//@ diag: 23:9 error "C-style cast from 'int (A::*)()' to 'aip' (aka 'int A::*') is not allowed"
//@ diag: 25:19 error "constexpr variable 'z' must be initialized by a constant expression"
struct A { int a; long l; int f(); };
typedef int A::*aip;
typedef long A::*alp;
typedef int (A::*F0)();
const int A::*ca = &A::a;
// reinterpret_cast casts away no constness ([expr.reinterpret.cast]/2).
aip m = reinterpret_cast<aip>(reinterpret_cast<const long A::*>(ca));
long A::*e = reinterpret_cast<long A::*>(ca);
// A data member pointer never becomes a member function pointer.
F0 x = (F0)&A::a;
aip y = (aip)&A::f;
// Not a constant expression ([expr.const]/10.15).
constexpr alp z = (alp)&A::a;
