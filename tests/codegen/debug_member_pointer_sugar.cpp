// Debug info (WP-1414): a pointer to member function keeps the sugar of
// its return and parameter types (clang's `CreateType(const
// MemberPointerType *)` takes `castAs<FunctionProtoType>`, which strips
// only the top-level sugar; [dcl.mptr]): `Int` and `Long` stay typedefs,
// so the subroutine type is the member's own. A pointer to data member
// keeps its typedef too. clang 19 and 22 agree.
//@ args: -g
using Int = int;
typedef long Long;
struct S {
  Int f(Long, const Int *);
  Int d;
};
typedef Int Fn(Long);
Int (S::*pm)(Long, const Int *) = &S::f;
Int S::*pd = &S::d;
using MF = Int (S::*)(Long);
MF mf;
Fn S::*pf;
int use() { return 0; }
