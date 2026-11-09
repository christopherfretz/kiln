// [dcl.constexpr]/3 (C++20; removed by P2448R2 in C++23): a constexpr
// function's return type and parameter types are literal types (clang
// `CheckConstexprReturnType` / `CheckConstexprParameterTypes`). Found while
// probing WP-455: kiln accepts the parameter and reports the return type as
// never-constant instead.
//@ xfail(kiln): WP-455 follow-up, no literal-type check of a constexpr function's return and parameter types (err_constexpr_non_literal_return/_param)
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 16:14 error "constexpr function's return type 'NL' is not a literal type"
//@ diag: 15:8 note "'NL' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 17:19 error "constexpr function's 1st parameter type 'NL' is not a literal type"
//@ diag: 15:8 note "'NL' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"

struct NL { NL(); };
constexpr NL f() { return NL(); }
constexpr int g(NL) { return 1; }
