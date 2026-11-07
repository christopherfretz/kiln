// [expr.prim.lambda.closure]/10 (P2900R14), Example 6: a lambda that would
// implicitly capture a local entity only for references in its call
// operator's function contract assertions or in assertion-statements in
// its body is ill-formed (adding a contract assertion cannot add a
// capture, Note 4). A namespace-scope variable is not captured (f1), an
// explicit capture is fine (f3), a reference outside the assertions
// before or after it makes the capture (f5, f8, f11), and a nested
// lambda's own local is no capture of the enclosing one (f6). Each entity
// once per lambda; a nested lambda whose only reference is its assertion
// makes both lambdas ill-formed, reported once (f9). GCC 16.1 reports the
// same, in the same words.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ diag: 30:21 error "'i' is not implicitly captured by a contract assertion"
//@ diag: 29:7 note "'i' declared here"
//@ diag: 32:35 error "'i' is not implicitly captured by a contract assertion"
//@ diag: 29:7 note "'i' declared here"
//@ diag: 36:41 error "'i' is not implicitly captured by a contract assertion"
//@ diag: 29:7 note "'i' declared here"
//@ diag: 37:23 error "'i' is not implicitly captured by a contract assertion"
//@ diag: 29:7 note "'i' declared here"
//@ diag: 39:41 error "'i' is not implicitly captured by a contract assertion"
//@ diag: 29:7 note "'i' declared here"
static int i = 0;
void test() {
  auto f1 = [=] pre(i > 0) {};
  int i = 1;
  auto f2 = [=] pre(i > 0) {};
  auto f3 = [i] pre(i > 0) {};
  auto f4 = [=] { contract_assert(i > 0); };
  auto f5 = [=] { contract_assert(i > 0); (void)i; };
  auto f6 = [=] pre([] { bool x = true; return [=] { return x; }(); }()) {};
  auto f8 = [=] { (void)i; contract_assert(i > 0); };
  auto f9 = [&] { [&] { contract_assert(i > 0); }(); };
  auto f10 = [=] post(i > 0) {};
  auto f11 = [=] pre(i > 0) { return i; };
  auto f12 = [&](int) { contract_assert(i > 0); };
}
