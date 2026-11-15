// [namespace.udecl]/11, [basic.scope.scope]/4, [dcl.typedef]/4 (WP-1448): a
// typedef-name declared after a using-declarator that named a typedef-name
// conflicts with it only when it denotes a different type; diagnosed as a
// typedef redefinition against the target (as clang). In a class a member
// typedef-name is not redeclared ([class.mem.general]/6).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:16 error "typedef redefinition with different types ('long' vs 'int')"
//@ diag: 18:42 note "previous definition is here"
//@ diag: 29:33 error "typedef redefinition with different types ('long' vs 'int')"
//@ diag: 18:27 note "previous definition is here"
//@ diag: 34:16 error "typedef redefinition with different types ('char' vs 'int')"
//@ diag: 18:42 note "previous definition is here"
//@ diag: 39:16 error "typedef redefinition with different types ('long' vs 'int')"
//@ diag: 36:27 note "previous definition is here"
//@ diag: 41:15 error "redefinition of 'U'"
//@ diag: 36:42 note "previous definition is here"
namespace A { typedef int T; typedef int U; using V = int; struct S {}; typedef S SS; }
namespace B {
  using A::T;
  typedef int T;   // the same type: valid
  using A::SS;
  typedef A::S SS; // valid
  using A::V;
  using V = int;   // valid
  using A::U;
  typedef long U;  // another type
}
namespace C { using A::T; using T = long; }
void f() {
  using A::T;
  typedef int T;   // valid
  using A::U;
  typedef char U;
}
struct Base { typedef int T; typedef int U; };
struct D : Base {
  using Base::T;
  typedef long T;
  using Base::U;
  typedef int U;   // a member typedef is never redeclared
};
