// [expr.prim.this]/2, [expr.prim.id.unqual]/7 (P2900R14): in a contract
// predicate of a member function `this` points to const, so a non-static
// data member is not modifiable and a non-const member function is not
// callable through it. GCC 16.1 rejects the same expressions.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ diag: 18:17 error "cannot modify 'q' in a contract predicate: it is implicitly const"
//@ diag: 19:18 error "cannot modify 'q' in a contract predicate: it is implicitly const"
//@ diag: 20:17 error "'this' argument to member function 'set' has type 'const T', but function is not marked const"
//@ diag: 16:8 note "'set' declared here"
//@ diag: 22:32 error "cannot modify 'q' in a contract predicate: it is implicitly const"
struct T {
  int q;
  bool set() { return ++q != 0; }
  bool ok() const { return q != 0; }
  void m1() pre(++q);
  void m2() pre((++this->q, true));
  void m3() pre(set());
  void m4() pre(ok());
  void m5() { contract_assert(q++); }
};
