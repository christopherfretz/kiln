// [dcl.attr.noreturn]/2: a `return` statement in a lambda whose call
// operator has a GNU `noreturn` function type is an error (clang
// `ActOnCapScopeReturnStmt`, `err_noreturn_lambda_has_return_expr`),
// with a deduced or a trailing return type alike. `[[noreturn]]` is a
// declaration attribute, not part of the type: its `return` is
// undiagnosed (undefined behavior at run time). kiln accepted both.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:44 error "lambda declared 'noreturn' should not return"
//@ diag: 15:51 error "lambda declared 'noreturn' should not return"
//@ diag: 17:48 error "lambda declared 'noreturn' should not return"
auto b3 = []() __attribute__((noreturn)) { return; };
auto b4 = [] [[noreturn]] () { return; };
auto b5 = [] [[noreturn]] () -> int { return 1; };
auto b6 = []() __attribute__((noreturn)) -> int { return 1; };
template <class T> void t() {
  auto l = [](T v) __attribute__((noreturn)) { return v; };
}
