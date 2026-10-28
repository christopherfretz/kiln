// [cpp.pragma]/1: a pragma the implementation does not recognize is ignored
// (clang warns under -Wunknown-pragmas, at the first unrecognized token).
//@ expect: compile-ok
//@ args: -Wunknown-pragmas
//@ strict-diags
//@ diag: 14:9 warning "unknown pragma ignored"
//@ diag: 15:13 warning "unknown pragma ignored"
//@ diag: 16:15 warning "unknown pragma ignored"
//@ diag: 17:9 warning "unknown pragma ignored"
//@ diag: 18:8 warning "unknown pragma ignored"
//@ diag: 19:9 warning "unknown pragma ignored"
#define EMPTY
#define FOO kiln_frobnicate
#pragma kiln_frobnicate 1 2 3
#pragma GCC frobnicate
#pragma clang frobnicate
#pragma FOO
#pragma
#pragma EMPTY
int x;
