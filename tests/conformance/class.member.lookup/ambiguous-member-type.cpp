// [class.member.lookup]/3, /6: a member type name found in two base class
// subobjects is ambiguous, used as a decl-specifier or to start a
// statement; each candidate is noted as a "member type" (a typedef by
// its underlying type); typedefs of one type are the same member
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:3 error "member 'T' found in multiple base classes of different types"
//@ diag: 17:24 note "member type 'int' found by ambiguous name lookup"
//@ diag: 18:25 note "member type 'long' found by ambiguous name lookup"
//@ diag: 21:3 error "member 'N' found in multiple base classes of different types"
//@ diag: 17:34 note "member type 'P::N' found by ambiguous name lookup"
//@ diag: 18:35 note "member type 'Q::N' found by ambiguous name lookup"
//@ diag: 22:14 error "member 'T' found in multiple base classes of different types"
//@ diag: 17:24 note "member type 'int' found by ambiguous name lookup"
//@ diag: 18:25 note "member type 'long' found by ambiguous name lookup"
//@ diag: 22:29 error "use of undeclared identifier 'u'"
struct P { typedef int T; struct N {}; };
struct Q { typedef long T; struct N {}; };
struct R : P, Q {
  T t;
  N n;
  void f() { T u = 0; (void)u; }
};
struct V { typedef int T; };
struct W { typedef int T; };
struct Z : V, W { T ok; };
