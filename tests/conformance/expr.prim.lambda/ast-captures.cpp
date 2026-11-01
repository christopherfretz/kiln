//@ expect: ast
//@ ast-filter: lamtest
// [expr.prim.lambda.capture]: explicit, implicit and init captures. A
// capture's member is an unnamed FieldDecl of the closure; names in the
// body refer to the captured entity (refers_to_enclosing_variable).
int lamtest(int a, int b) {
  const int n = 4;
  auto c = [a, &b] { return a + b; };
  auto d = [=] { return a * n; };
  auto e = [&] { b += a; };
  auto f = [x = a + 1, &y = b] { return x + y; };
  auto g = [a]() mutable { return ++a; };
  e();
  return c() + d() + f() + g();
}
