// [basic.def.odr]/1, [namespace.def.general]/2: a variable redefined in a
// reopened namespace (or redeclared there with another type) is
// diagnosed once; the invalid redeclaration does not become a second
// entity, so later uses of the name, qualified, unqualified, at the start
// of a statement or through a using-directive, find the first one rather
// than an ambiguous set (clang marks the redeclaration invalid and
// `LookupResult::resolveKind` ignores it). Matches clang 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:19 error "redefinition of 'y'"
//@ diag: 20:19 note "previous definition is here"
//@ diag: 22:22 error "redefinition of 'w' with a different type: 'double' vs 'int'"
//@ diag: 20:26 note "previous definition is here"
//@ diag: 23:66 error "redefinition of 'c'"
//@ diag: 23:29 note "previous definition is here"
//@ diag: 24:19 error "redefinition of 'q'"
//@ diag: 20:33 note "previous definition is here"
//@ diag: 24:23 error "use of undeclared identifier 'undeclared'"
//@ diag: 30:6 error "cannot initialize a variable of type 'int *' with an rvalue of type 'const int *'"
namespace N { int y; int w; int q = 1; }
namespace N { int y = 1; }
namespace N { double w = 2; }
namespace N { constexpr int c = 1; } namespace N { constexpr int c = 2; }
namespace N { int q = undeclared; }
int z = N::y + N::w + N::q;
void f() { N::y = 3; N::w++; }
namespace N { void g() { y = 4; y++; decltype(y) t = y; (void)t; } }
static_assert(N::c == 1);
using namespace N;
int *p = &c;
void h() { w = 6; y = 7; q = 8; }
