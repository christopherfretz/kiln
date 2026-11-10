// [expr.prim.id.unqual]/7, [expr.prim.lambda.capture]/10 (P2900R14): a
// by-copy capture of a mutable lambda declared inside a contract
// predicate is a member of the closure, not the outer variable, so the
// lambda may modify it; reading the outer entities is fine. GCC 16.1
// makes an explicit by-copy capture (`[n]`) const but not an implicit one
// (`[=]`); kiln follows the capture rules (spec/contracts.md §9), so there
// is no oracle.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ xfail(gcc): GCC 16.1 makes an explicit by-copy capture of a mutable lambda in a predicate const (spec/contracts.md §9)
void k(int a) {
  int n = 0;
  contract_assert([=]() mutable { ++n; return ++a; }());
  contract_assert([n]() mutable { return ++n; }());
  contract_assert([&] { return a + n; }() >= 0);
}
