// [expr.prim.lambda.general], [temp.variadic]/5 and /12: a pack declared
// by a generic lambda and named outside an expansion in the lambda's own
// body is diagnosed where the enclosing construct ends: a full-expression
// or condition ("expression", at its begin), a static assertion, a
// default argument of a local function, an exception-declaration's type
// (at the declarator-id), a range or a case value. A default argument of
// a nested lambda's parameter makes that lambda-expression contain the
// pack (diagnosed at the lambda's full-expression); expanded, it is valid.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++26
//@ args: -Wno-unused-value -ferror-limit=0
//@ diag: 34:20 error "expression contains unexpanded parameter pack 'xs'"
//@ diag: 35:27 error "expression contains unexpanded parameter pack 'xs'"
//@ diag: 36:28 error "initializer contains unexpanded parameter pack 'xs'"
//@ diag: 37:32 error "initializer contains unexpanded parameter pack 'xs'"
//@ diag: 38:24 error "expression contains unexpanded parameter pack 'xs'"
//@ diag: 39:28 error "expression contains unexpanded parameter pack 'xs'"
//@ diag: 40:33 error "expression contains unexpanded parameter pack 'xs'"
//@ diag: 41:29 error "initializer contains unexpanded parameter pack 'xs'"
//@ diag: 42:20 error "expression contains unexpanded parameter pack 'xs'"
//@ diag: 43:20 error "expression contains unexpanded parameter pack 'xs'"
//@ diag: 44:25 error "default argument contains unexpanded parameter pack 'Ts'"
//@ diag: 45:33 error "default argument contains unexpanded parameter pack 'xs'"
//@ diag: 46:42 error "expression contains unexpanded parameter pack 'ys'"
//@ diag: 47:34 error "static assertion contains unexpanded parameter pack 'xs'"
//@ diag: 48:46 error "exception type contains unexpanded parameter pack 'xs'"
//@ diag: 49:47 error "exception type contains unexpanded parameter pack 'xs'"
//@ diag: 50:33 error "expression contains unexpanded parameter pack 'xs'"
//@ diag: 51:33 error "expression contains unexpanded parameter pack 'xs'"
//@ diag: 52:38 error "expression contains unexpanded parameter pack 'xs'"
//@ diag: 53:20 error "expression contains unexpanded parameter pack 'xs'"
void a() {
  [](auto... xs) { (void)xs; };
  [](auto... xs) { return xs; };
  [](auto... xs) { int y = xs; };
  [](auto... xs) { int arr[] = {xs}; };
  [](auto... xs) { if (xs) {} };
  [](auto... xs) { switch (xs) {} };
  [](auto... xs) { do {} while (xs); };
  [](auto... xs) { auto l = [xs] {}; };
  [](auto... xs) { [](int = sizeof(xs)) {}; };
  [](auto... xs) { [](int = sizeof(xs)) {}(); };
  []<class... Ts>(int = sizeof(Ts)) {};
  [](auto... xs) { void f(int = sizeof(xs)); };
  [](auto... xs) { [...ys = xs] { return [ys] {}; }; };
  [](auto... xs) { static_assert(sizeof(xs) > 0); };
  [](auto... xs) { try {} catch (decltype(xs)) {} };
  [](auto... xs) { try {} catch (decltype(xs) e) { (void)e; } };
  [](auto... xs) { for (int i : xs) {} };
  [](auto... xs) { for (int i : {xs}) {} };
  [](auto... xs) { switch (1) { case sizeof(xs): break; } };
  [](auto... xs) { (void)[&] { return xs; }; };
}
// Expanded: valid, as is `sizeof...` in a nested lambda's default argument.
void b() {
  [](auto... xs) { [](int i = sizeof...(xs)) { return i; }; };
  [](auto... xs) { [...ys = xs] { (void)(ys + ...); }; };
  [](auto... xs) { ([](int = sizeof(xs)) {}(), ...); };
}
static_assert([](auto... xs) {
  return ([](int i = sizeof(xs)) { return i; }() + ... + 0);
}(char(1), short(2)) == 3);
