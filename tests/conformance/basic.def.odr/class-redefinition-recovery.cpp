// [basic.def.odr]/1, [class.pre], [dcl.enum]: a class or enumeration
// redefinition is diagnosed once; later uses of the name find the first
// definition and are not ambiguous, also in a reopened (unnamed, inline)
// namespace, a class and a function, and for a class template. The
// redefinition's body is not checked and its enumerators are not
// declared (spec/sema.md §4.4).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:8 error "redefinition of 'X'"
//@ diag: 28:8 note "previous definition is here"
//@ diag: 32:22 error "redefinition of 'Y'"
//@ diag: 31:22 note "previous definition is here"
//@ diag: 35:20 error "redefinition of 'A'"
//@ diag: 34:20 note "previous definition is here"
//@ diag: 38:29 error "redefinition of 'B'"
//@ diag: 37:29 note "previous definition is here"
//@ diag: 41:27 error "redefinition of 'C'"
//@ diag: 40:27 note "previous definition is here"
//@ diag: 45:10 error "redefinition of 'I'"
//@ diag: 44:10 note "previous definition is here"
//@ diag: 50:10 error "redefinition of 'L'"
//@ diag: 49:10 note "previous definition is here"
//@ diag: 54:6 error "redefinition of 'E'"
//@ diag: 53:6 note "previous definition is here"
//@ diag: 55:10 error "use of undeclared identifier 'e2'"
//@ diag: 57:26 error "no member named 'b' in 'X'"

struct X { int a; };
struct X { int b; static_assert(false); };
X x;
namespace N { struct Y {}; }
namespace N { struct Y {}; }
N::Y y;
namespace { struct A {}; }
namespace { struct A { int z; }; }
A a;
inline namespace I { struct B {}; }
inline namespace I { struct B {}; }
B b;
template <class T> struct C {};
template <class T> struct C { undeclared u; };
C<int> c;
struct O {
  struct I {};
  struct I { int q; };
  I i;
};
void f() {
  struct L {};
  struct L { int z; };
  L l;
}
enum E { e1 };
enum E { e2 };
int ee = e2;
E e = e1;
int g() { return x.a + x.b; }
