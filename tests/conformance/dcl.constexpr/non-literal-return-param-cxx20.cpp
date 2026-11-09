// [dcl.constexpr]/3 (C++20; removed by P2448R2 in C++23): a constexpr
// function's return type and parameter types are literal types (clang
// `CheckConstexprReturnType` / `CheckConstexprParameterTypes`). Found while
// probing WP-455 (kiln accepted the parameter and reported the return type
// as never-constant instead); fixed by WP-1123.
//
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
