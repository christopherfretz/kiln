// [basic.lookup.general]/1, [namespace.udir]/6: a type name whose lookup
// finds declarations of distinct entities is ambiguous (typedefs of one
// type are not); a decl-specifier recovers with the first type found, a
// statement starting with the ambiguous name is skipped (clang's
// ClassifyName); a class beside a variable from another namespace is
// not hidden
//@ expect: compile-fail
//@ diag: 51:1 error "reference to 'X' is ambiguous"
//@ diag: 47:22 note "candidate found by name lookup is 'A::X'"
//@ diag: 48:22 note "candidate found by name lookup is 'B::X'"
//@ diag: 54:3 error "reference to 'X' is ambiguous"
//@ diag: 47:22 note "candidate found by name lookup is 'A::X'"
//@ diag: 48:22 note "candidate found by name lookup is 'B::X'"
//@ diag: 55:3 error "use of undeclared identifier 'y'"
//@ diag: 56:3 error "reference to 'T' is ambiguous"
//@ diag: 47:48 note "candidate found by name lookup is 'A::T'"
//@ diag: 48:49 note "candidate found by name lookup is 'B::T'"
//@ diag: 57:3 error "reference to 'X' is ambiguous"
//@ diag: 47:22 note "candidate found by name lookup is 'A::X'"
//@ diag: 48:22 note "candidate found by name lookup is 'B::X'"
//@ diag: 58:9 error "use of undeclared identifier 'p'"
//@ diag: 61:12 error "reference to 'X' is ambiguous"
//@ diag: 47:22 note "candidate found by name lookup is 'A::X'"
//@ diag: 48:22 note "candidate found by name lookup is 'B::X'"
//@ diag: 61:24 error "reference to 'X' is ambiguous"
//@ diag: 47:22 note "candidate found by name lookup is 'A::X'"
//@ diag: 48:22 note "candidate found by name lookup is 'B::X'"
//@ diag: 62:7 error "reference to 'X' is ambiguous"
//@ diag: 47:22 note "candidate found by name lookup is 'A::X'"
//@ diag: 48:22 note "candidate found by name lookup is 'B::X'"
//@ diag: 63:16 error "reference to 'X' is ambiguous"
//@ diag: 47:22 note "candidate found by name lookup is 'A::X'"
//@ diag: 48:22 note "candidate found by name lookup is 'B::X'"
//@ diag: 64:27 error "reference to 'X' is ambiguous"
//@ diag: 47:22 note "candidate found by name lookup is 'A::X'"
//@ diag: 48:22 note "candidate found by name lookup is 'B::X'"
//@ diag: 64:31 error "reference to 'X' is ambiguous"
//@ diag: 47:22 note "candidate found by name lookup is 'A::X'"
//@ diag: 48:22 note "candidate found by name lookup is 'B::X'"
//@ diag: 65:31 error "reference to 'X' is ambiguous"
//@ diag: 47:22 note "candidate found by name lookup is 'A::X'"
//@ diag: 48:22 note "candidate found by name lookup is 'B::X'"
//@ diag: 69:12 error "reference to 'Y' is ambiguous"
//@ diag: 66:22 note "candidate found by name lookup is 'D::Y'"
//@ diag: 67:19 note "candidate found by name lookup is 'E::Y'"
//@ diag: 69:17 error "use of undeclared identifier 'z'"
namespace A { struct X { int a; }; typedef int T; typedef int U; }
namespace B { struct X { int b; }; typedef long T; typedef int U; }
using namespace A;
using namespace B;
X x;
U u;
void f() {
  X y;
  y.a = 1;
  T t = 0;
  X *p = nullptr;
  (void)p;
  U v = 0;
}
struct S { X m; void g(X); };
int h(X);
int s = sizeof(X);
void k() { int n = sizeof(X); X(1); }
template<class W> struct Tm { X w; };
namespace D { struct Y {}; }
namespace E { int Y; }
using namespace D; using namespace E;
void m() { Y z; z = 0; }
