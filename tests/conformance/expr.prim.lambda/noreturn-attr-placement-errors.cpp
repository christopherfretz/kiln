// [expr.prim.lambda.general]/1, [dcl.fct]/1: the placements a
// lambda-declarator rejects (clang `ParseLambdaSpecifiers`). A GNU
// attribute after the lambda-specifiers or the noexcept-specifier, or
// ahead of the parameter list, ends the declarator: expected body of
// lambda expression. An attribute-specifier-seq after the
// noexcept-specifier or the trailing return type appertains to a type,
// where `[[noreturn]]` and `[[nodiscard]]` cannot appear. kiln accepted
// all but the trailing-return one, applying them to the call operator.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++23
//@ diag: 19:36 error "expected body of lambda expression"
//@ diag: 20:35 error "expected body of lambda expression"
//@ diag: 21:52 error "expected body of lambda expression"
//@ diag: 22:38 error "'noreturn' attribute cannot be applied to types"
//@ diag: 23:29 error "'noreturn' attribute cannot be applied to types"
//@ diag: 24:37 error "'noreturn' attribute cannot be applied to types"
//@ diag: 25:30 error "'nodiscard' attribute cannot be applied to types"
void a1() { auto l = []() noexcept __attribute__((noreturn)) {}; }
void a9() { auto l = []() mutable __attribute__((noreturn)) {}; }
void a12() { auto l = [] __attribute__((noreturn)) () {}; }
void a5() { auto l = []() noexcept [[noreturn]] {}; }
void a6() { auto l = []() [[noreturn]] {}; }
void a7() { auto l = []() -> void [[noreturn]] {}; }
void a11() { auto l = []() [[nodiscard]] { return 1; }; }
