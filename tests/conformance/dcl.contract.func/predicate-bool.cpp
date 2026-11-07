// [dcl.contract.func]/2, [stmt.contract.assert]/2 (P2900R14): a predicate
// is contextually converted to bool, so an explicit conversion operator
// is usable and a type without one is an error. GCC 16.1 rejects the same
// two predicates.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ diag: 14:18 error "value of type 'const N' is not contextually convertible to 'bool'"
//@ diag: 15:32 error "value of type 'const N' is not contextually convertible to 'bool'"
struct E { explicit operator bool() const { return true; } };
struct N {};
void f1(E e) pre(e) {}
void f2(N n) pre(n) {}
void f3(N n) { contract_assert(n); }
