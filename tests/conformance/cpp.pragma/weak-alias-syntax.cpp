// [cpp.pragma]: a malformed '#pragma weak alias = target' is ignored with
// a warning (clang PragmaWeakHandler); a target never declared as a
// function or variable is reported at the end of the translation unit at
// the pragma's alias, a C++ function declared later included (clang
// waits for an extern "C" declaration); a later class is not a function
// or variable (clang ProcessPragmaWeak). WP-1399.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 16:18 warning "expected identifier in '#pragma weak' - ignored"
//@ diag: 17:19 warning "expected identifier in '#pragma weak' - ignored"
//@ diag: 18:22 warning "extra tokens at end of '#pragma weak' - ignored"
//@ diag: 19:14 warning "weak identifier 'never_declared' never declared"
//@ diag: 20:14 warning "weak identifier 'cxx_late' never declared"
//@ diag: 22:14 warning "'weak' attribute only applies to variables and functions"
//@ diag: 24:14 warning "weak identifier 'plain_never' never declared"
#pragma weak a1 =
#pragma weak a2 = 3
#pragma weak a3 = t3 x
#pragma weak a4 = never_declared
#pragma weak a5 = cxx_late
int cxx_late();
#pragma weak a6 = obj
struct obj {};
#pragma weak plain_never
