// [expr.prim.lambda.closure]/10 (P2900R14), Example 6 #2 (f7): a lambda
// whose implicit capture is needed only by a lambda inside its own
// precondition is ill-formed, though the inner lambda references the
// entity outside any assertion of its own; `*this` is a local entity
// too ([basic.pre]/7), captured by `[=]` or `[&]` only for a contract
// assertion; in a template the check runs on the instantiation (each
// once), also in a generic lambda's body (whose two specializations
// report once); a reference that is no odr-use (a constant's value,
// [basic.def.odr]/5) captures nothing, so no diagnostic. GCC 16.1
// accepts f7 and the `this` cases and rejects the constant `n`,
// against the wording (spec/contracts.md §9).
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ args: -Wno-deprecated-this-capture
//@ diag: 32:34 error "'y' is not implicitly captured by a contract assertion"
//@ diag: 31:7 note "'y' declared here"
//@ diag: 33:32 error "'i' is not implicitly captured by a contract assertion"
//@ diag: 30:12 note "'i' declared here"
//@ diag: 34:42 error "'i' is not implicitly captured by a contract assertion"
//@ diag: 30:12 note "'i' declared here"
//@ diag: 35:21 error "'i' is not implicitly captured by a contract assertion"
//@ diag: 30:12 note "'i' declared here"
//@ diag: 43:22 error "'this' is not implicitly captured by a contract assertion"
//@ diag: 44:36 error "'this' is not implicitly captured by a contract assertion"
//@ diag: 49:20 error "'v' is not implicitly captured by a contract assertion"
//@ diag: 48:30 note "'v' declared here"
//@ diag: 50:42 error "'v' is not implicitly captured by a contract assertion"
//@ diag: 48:30 note "'v' declared here"
void h(int i) {
  int y = 2;
  auto f7 = [=] pre([=] { return y; }()) {};
  auto a = [=] pre([=] { (void)i; return true; }()) {};
  auto b = [=] pre([=] { contract_assert(i > 0); return true; }()) {};
  auto c = [&] post(i > 0) { return; };
  const int n = 3;
  auto d = [=] pre(n > 0) {};
  auto e = [=] pre(*&n > 0) { (void)&n; };
}
struct S {
  int m;
  void f() {
    auto l = [=] pre(m > 0) {};
    auto k = [&] { contract_assert(m > 0); };
    auto j = [this] pre(m > 0) {};
  }
};
template <class T> void tf(T v) {
  auto a = [=] pre(v > 0) {};
  auto c = [=](auto x) { contract_assert(v > x); };
  c(1);
  c(2L);
}
void g() { tf(1); }
