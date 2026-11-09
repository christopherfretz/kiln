// [dcl.constexpr]/3 (C++20): clang checks the body of a constexpr function
// (`CheckConstexprFunctionDefinition`) before the analysis-based warnings,
// which it skips once an error has been reported, so a function rejected
// for an indirect goto gets no -Wreturn-type for falling off
// its end. kiln runs the constexpr body checks at the same point (WP-455).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 12:27 error "statement not allowed in constexpr function"

constexpr int indirect(int x) {
  if (x) { void *p = &&l; goto *p; l: return 1; }
}
