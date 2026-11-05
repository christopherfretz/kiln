// clang's GNU-spelled `analyzer_noreturn` (Attr.td AnalyzerNoReturn,
// `__attribute__` spelling only) applies to functions and to
// function-pointer variables and takes no arguments; the `[[...]]`
// spellings are unknown attributes. The flow effect is tested in
// analyzer-noreturn.cpp, which must stay free of errors (an error
// suppresses the flow warnings).
//@ expect: compile-fail
//@ args: -Wall
//@ strict-diags
//@ diag: 15:23 warning "'analyzer_noreturn' attribute only applies to functions, methods and blocks"
//@ diag: 16:26 error "'analyzer_noreturn' attribute takes no arguments"
//@ diag: 17:3 warning "unknown attribute 'clang::analyzer_noreturn' ignored"
//@ diag: 18:3 warning "unknown attribute 'gnu::analyzer_noreturn' ignored"

int v1 __attribute__((analyzer_noreturn));
void f3() __attribute__((analyzer_noreturn(1)));
[[clang::analyzer_noreturn]] void f4();
[[gnu::analyzer_noreturn]] void f5();
