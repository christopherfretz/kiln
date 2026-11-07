// [stmt.contract.assert]/1-2, [stmt.pre]/1 (P2900R14): `contract_assert (
// predicate ) ;` is a statement: usable wherever a statement is (a
// compound statement, an unbraced if/loop body, a lambda body, a constexpr
// function, a template), with a predicate contextually converted to bool.
// GCC 16.1 agrees.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
struct E { explicit operator bool() const { return true; } };
void f(int x, E e, int* p) {
  contract_assert(x > 0);
  contract_assert(e);
  contract_assert(p);
  if (x) contract_assert(x != 1);
  for (int i = 0; i < x; ++i) contract_assert(i < x);
  auto l = [x] { contract_assert(x > 0); return x; };
  (void)l;
}
constexpr int c(int x) { contract_assert(x >= 0); return x; }
template <class T> T t(T v) { contract_assert(v == v); return v; }
int u = t(1) + c(0);
