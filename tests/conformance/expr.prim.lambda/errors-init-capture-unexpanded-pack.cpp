// [expr.prim.lambda.capture]/17, [temp.variadic]/5,12: an init-capture
// `[x = ts]` naming a pack it does not expand makes the lambda-expression
// contain the pack, diagnosed at the enclosing full-expression or
// initializer; `[...xs = ts]` expands it. In a lambda body, a pack the
// lambda itself declares (an init-capture pack, a parameter pack, a
// template parameter pack) is diagnosed there, also when a nested lambda
// names it; an enclosing scope's pack is the lambda's.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 22:12 error "initializer contains unexpanded parameter pack 'ts'"
//@ diag: 23:3 error "expression contains unexpanded parameter pack 'ts'"
//@ diag: 25:24 error "expression contains unexpanded parameter pack 'xs'"
//@ diag: 26:31 error "expression contains unexpanded parameter pack 'xs'"
//@ diag: 27:33 error "initializer contains unexpanded parameter pack 'xs'"
//@ diag: 28:3 error "expression contains unexpanded parameter pack 'ts'"
//@ diag: 31:26 error "expression contains unexpanded parameter pack 'as'"
//@ diag: 32:34 error "initializer contains unexpanded parameter pack 'as'"
//@ diag: 33:40 error "declaration type contains unexpanded parameter pack 'Us'"
//@ diag: 34:33 error "expression contains unexpanded parameter pack 'as'"
template <class... Ts> void f(Ts... ts) {
  auto a = [x = ts] { return x; };
  (void)[x = ts] {};
  auto b = [...xs = ts] { return sizeof...(xs); };
  (void)[...xs = ts] { (void)[y = xs] {}; };
  (void)[...xs = ts] { return [y = xs] { return y; }; };
  (void)[...xs = ts] { auto q = [y = xs] { return y; }; };
  (void)[&] { (void)[y = ts] {}; };
}
void h() {
  (void)[](auto... as) { (void)[x = as] {}; };
  (void)[](auto... as) { int x = as; };
  (void)[]<class... Us>(Us... us) { Us u; };
  (void)[](auto... as) { return [&] { as; }; };
}
// Expanded around the lambda, or inside it: fine.
template <class... Ts> constexpr int g(Ts... ts) {
  int a = [...xs = ts] { return (xs + ...); }();
  int b = [...xs = ts] { return ([y = xs] { return y; }() + ...); }();
  int c = [&] { return ([y = ts] { return y; }() + ...); }();
  int d = [](auto... as) { return ([&] { return as; }() + ...); }(ts...);
  int e = ([x = ts] { return x; }() + ...);
  return a + b + c + d + e;
}
static_assert(g(1, 2, 3) == 30);
