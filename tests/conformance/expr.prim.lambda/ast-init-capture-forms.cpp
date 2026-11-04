//@ expect: ast
//@ ast-filter: lamtest
// [expr.prim.lambda.capture]/6: an init-capture declares a variable as if
// by `auto name initializer` -- `= e`, `(e)` or `{e}` -- and its closure
// member is initialized from that initializer: the braced form keeps its
// InitListExpr, the parenthesized form is the bare expression, and `&r(a)`
// binds a reference member.
int lamtest(int a) {
  auto l = [x{1}, y(2), z = a, &r(a)] { return x + y + z + r; };
  return l();
}
