// [dcl.attr.noreturn]/2: control reaching the end of a lambda whose call
// operator is GNU `noreturn` is an error for a lambda (clang
// `err_noreturn_lambda_has_return_expr`), not the
// -Winvalid-noreturn warning of a function. kiln warned.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:86 error "lambda declared 'noreturn' should not return"
void f(int x) { auto l = [x]() __attribute__((noreturn)) { if (x) __builtin_abort(); }; }
