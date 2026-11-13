// [expr.prim.lambda.capture]/7, [basic.pre]/7: a member of a block-scope
// anonymous union names a member of an unnamed variable, which a lambda
// cannot capture implicitly, by reference or by copy, also in a template
// (as clang 19 and 22 and GCC). An unevaluated use and a static anonymous
// union need no capture. Named in a capture list it names no variable,
// and the unnamed variable then has no capture fix-it notes (clang).
// Found by WP-1389 while writing a -g test; WP-1405.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:25 error "unnamed variable cannot be implicitly captured in a lambda expression"
//@ diag: 23:3 note "declared here"
//@ diag: 26:26 error "unnamed variable cannot be implicitly captured in a lambda expression"
//@ diag: 23:3 note "declared here"
//@ diag: 34:25 error "unnamed variable cannot be implicitly captured in a lambda expression"
//@ diag: 33:3 note "declared here"
//@ diag: 40:14 error "'i' in capture list does not name a variable"
//@ diag: 40:26 error "variable '' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 39:3 note "'' declared here"
//@ diag: 40:12 note "lambda expression begins here"
//@ diag: 40:13 note "default capture by value"
//@ diag: 40:13 note "default capture by reference"
int f() {
  union { int i; float fl; };
  i = 3;
  auto l = [&] { return i; };
  auto l2 = [=] { return i; };
  auto l3 = [&] { return sizeof(i) + noexcept(i); };
  static union { int si; };
  auto l4 = [&] { return si; };
  return l() + l2() + l3() + l4();
}
template <class T> int h() {
  union { T i; };
  auto l = [&] { return i; };
  return l();
}
int x = h<int>();
int g() {
  union { int i; };
  auto l = [&i] { return i; };
  return l();
}
