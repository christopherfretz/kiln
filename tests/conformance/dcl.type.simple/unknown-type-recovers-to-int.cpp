// [dcl.type.simple]/2, [dcl.ref]/5, [dcl.array]/5: a decl-specifier naming no
// type (`S::c` with no member `c`, `N::t`, an undeclared `u1`) is reported
// once and the declaration recovers with type `int`, as clang's
// `ParseImplicitInt`. The declaration is invalid, yet an uninitialized
// reference still reports "requires an initializer" and an array of unknown
// bound "needs an explicit size"; an initializer of an invalid variable is
// not checked. An invalid parameter still takes part in overload resolution
// as `int &` ("expects an lvalue").
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:4 error "no type named 'c' in 'S'"
//@ diag: 35:7 error "declaration of reference variable 'r1' requires an initializer"
//@ diag: 36:4 error "no type named 't' in namespace 'N'"
//@ diag: 36:7 error "declaration of reference variable 'r2' requires an initializer"
//@ diag: 37:1 error "unknown type name 'u1'"
//@ diag: 37:5 error "declaration of reference variable 'r3' requires an initializer"
//@ diag: 38:4 error "no type named 'c' in 'S'"
//@ diag: 39:4 error "no type named 't' in namespace 'N'"
//@ diag: 40:11 error "no type named 'c' in 'S'"
//@ diag: 41:4 error "no type named 'c' in 'S'"
//@ diag: 41:6 error "definition of variable with array type needs an explicit size or an initializer"
//@ diag: 42:5 error "definition of variable with array type needs an explicit size or an initializer"
//@ diag: 43:15 error "constexpr variable 'a3' must be initialized by a constant expression"
//@ diag: 44:15 error "no type named 't' in namespace 'N'"
//@ diag: 45:11 error "no type named 'c' in 'S'"
//@ diag: 46:8 error "unknown type name 'u2'"
//@ diag: 48:3 error "no matching function for call to 'h'"
//@ diag: 45:6 note "candidate function not viable: expects an lvalue for 1st argument"
//@ diag: 49:3 error "no matching function for call to 'k'"
//@ diag: 46:6 note "candidate function not viable: expects an lvalue for 1st argument"
//@ diag: 53:3 error "no matching function for call to 'h'"
//@ diag: 45:6 note "candidate function not viable: no known conversion from 'long' to 'int &' for 1st argument"
struct S {};
namespace N {}
S::c &r1;
N::t &r2;
u1 &r3;
S::c &r4 = 1;
N::t x1 = 1;
extern S::c &r5;
S::c a1[];
int a2[];
constexpr int a3[];
struct M { N::t &m; };
void h(S::c &p);
void k(u2 &p, int);
void use() {
  h(1);
  k(2, 3);
  int i = 0;
  h(i);
  long l = 0;
  h(l);
  int a4[];
  static int a5[];
}
