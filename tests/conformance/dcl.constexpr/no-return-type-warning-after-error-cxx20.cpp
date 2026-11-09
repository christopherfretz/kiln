// [dcl.constexpr]/3 (C++20): clang checks the body of a constexpr function
// (`CheckConstexprFunctionDefinition`) before the analysis-based warnings,
// which it skips once an error has been reported, so a function rejected
// for a non-literal local variable gets no -Wreturn-type for falling off
// its end. kiln runs the constexpr body checks at the same point (WP-455).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 13:39 error "variable of non-literal type 'NL' cannot be defined in a constexpr function before C++23"
//@ diag: 12:8 note "'NL' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"

struct NL { NL(); };
constexpr int non_literal(int x) { NL n; if (x) return 1; }
