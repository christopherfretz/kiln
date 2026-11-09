// [dcl.constexpr]/3 (C++17): a constexpr function's return type and
// parameter types are literal types, a lambda declared `constexpr`
// included ([expr.prim.lambda.closure]/5); one not so declared is simply not
// constexpr. clang `CheckConstexprReturnType` /
// `CheckConstexprParameterTypes`; before C++20 the note names a
// user-provided destructor (WP-1123).
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 22:13 error "constexpr function's return type 'D' is not a literal type"
//@ diag: 20:29 note "'D' is not literal because it has a user-provided destructor"
//@ diag: 23:23 error "constexpr function's 1st parameter type 'NL' is not a literal type"
//@ diag: 19:8 note "'NL' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 24:34 error "constexpr function's 2nd parameter type 'NL' is not a literal type"
//@ diag: 19:8 note "'NL' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 25:22 error "constexpr function's return type 'D' is not a literal type"
//@ diag: 20:29 note "'D' is not literal because it has a user-provided destructor"

struct NL { NL(); };
struct D { constexpr D() {} ~D() {} };

constexpr D ret() { return D(); }
constexpr int param(NL) { return 1; }
auto explicit_lambda = [](int, NL) constexpr { return 1; };
auto return_lambda = []() constexpr -> D { return D(); };

// Valid: `void` is literal from C++14; a lambda not declared constexpr
// is just not constexpr ([expr.prim.lambda.closure]/5).
constexpr void void_ok() {}
auto implicit_lambda = [](NL) { return 1; };
