// clang's GNU-spelled `analyzer_noreturn` (Attr.td AnalyzerNoReturn,
// `__attribute__` spelling only) applies to functions and to
// function-pointer variables and takes no arguments. The accepted
// spellings are in analyzer-noreturn.cpp.
//@ expect: compile-fail
//@ args: -Wall
//@ strict-diags
//@ diag: 11:23 warning "'analyzer_noreturn' attribute only applies to functions, methods and blocks"
//@ diag: 12:26 error "'analyzer_noreturn' attribute takes no arguments"

int v1 __attribute__((analyzer_noreturn));
void f3() __attribute__((analyzer_noreturn(1)));
