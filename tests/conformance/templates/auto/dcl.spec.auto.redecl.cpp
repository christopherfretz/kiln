// [dcl.spec.auto.general]/13: redeclarations of a function with a deduced
// return type use the same placeholder.
//@ strict-diags
auto f();
auto f() { return 1; }
decltype(auto) g();
decltype(auto) g() { return 2u; }
static_assert(__is_same(decltype(f()), int) && __is_same(decltype(g()), unsigned));
