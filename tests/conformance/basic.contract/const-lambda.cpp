// [expr.prim.id.unqual]/7 (P2900R14): const-ness applies to every
// id-expression in the predicate that names an entity declared outside
// it, also inside a lambda in the predicate (a by-reference capture
// names the outer entity, and `this` stays a pointer to const); only a
// by-copy capture of a mutable lambda declared in the predicate is a
// member of the closure and is modifiable. GCC 16.1 does not const-ify
// inside lambdas (spec/contracts.md §9), so there is no oracle.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ diag: 19:27 error "cannot modify 'm' in a contract predicate: it is implicitly const"
//@ diag: 20:24 error "cannot modify 'm' in a contract predicate: it is implicitly const"
//@ diag: 22:25 error "cannot modify 'glob' in a contract predicate: it is implicitly const"
//@ diag: 26:32 error "cannot modify 'i' in a contract predicate: it is implicitly const"
//@ xfail(gcc): GCC 16.1 does not make id-expressions inside a lambda in a predicate const (spec/contracts.md §9)
int glob;
struct S {
  int m;
  void f() pre([this] { m = 1; return true; }());
  void g() pre([&] { m = 1; return true; }());
};
void d() pre([&] { glob = 1; return true; }());
void k(int a) {
  int i = 0, n = 0;
  contract_assert([=, &i]() mutable { ++n; return ++a; }());
  contract_assert([&] { return ++i; }());
}
