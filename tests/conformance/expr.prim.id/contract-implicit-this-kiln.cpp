// [expr.prim.id.general]/2 (P2900R14), the cases where kiln follows the
// wording and GCC 16.1 does not: `sizeof(b)` is not potentially evaluated,
// so no `(*this).b` is formed (Example 1 of the paragraph: OK; GCC rejects
// it); a lambda in the predicate that names a member through its captured
// `this` makes that transformation in the predicate (GCC accepts it).
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ diag: 13:30 error "a precondition of a constructor cannot access a member through an implicit 'this'; write 'this->' explicitly"
struct C {
  bool b;
  C() pre(sizeof(b) > 0) {}
  C(int) pre([this] { return b; }()) {}
};
