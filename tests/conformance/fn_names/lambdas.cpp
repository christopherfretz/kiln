// Function-name spans (spec/ast.md §24.4): a closure in the scope is one
// synthesized scope entry over its spelling (clang 22's `(lambda)`), the call operator
// the name `operator()`; a generic lambda's invented parameter is an
// invented binding. The spelling follows the oracle (`-fclang-compat`),
// so each major has its own test. (A closure as a template argument,
// `(lambda at F:L:C)`, prints the input's path: crates/kiln_parse/tests/fn_names.rs.)
//@ fn-names: lambdas.names
//@ std: c++20
//@ requires: clang>=22
namespace ns {
void loc() {
  auto lam = [](int) { (void)__PRETTY_FUNCTION__; (void)__func__; };
  lam(1);
  auto gen = [](auto v) { (void)__PRETTY_FUNCTION__; return v; };
  gen(1);
}
}
