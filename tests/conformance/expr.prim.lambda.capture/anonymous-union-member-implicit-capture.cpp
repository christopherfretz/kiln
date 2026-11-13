// [expr.prim.lambda.capture]/7, [basic.pre]/7: a member of a block-scope
// anonymous union names a member of an unnamed variable, which a lambda
// cannot capture implicitly. Kiln accepts it; clang 19 and 22 and GCC
// reject it. Found by WP-1389 while writing a -g test.
//@ expect: compile-fail
//@ xfail(kiln): WP-1389 follow-up, implicit capture of a block-scope anonymous union member is accepted
//@ diag: 12:25 error "unnamed variable cannot be implicitly captured in a lambda expression"
//@ diag: 10:3 note "declared here"
int f() {
  union { int i; float fl; };
  i = 3;
  auto l = [&] { return i; };
  return l();
}
