// [cpp.cond]: an `#elif`, `#else` or `#endif` with no open conditional is
// an error. Each still gets a row, with no opener (`stray`) and no group
// (spec/pp.md "Preprocessing record", conditionals).
//@ expect: compile-fail
//@ strict-diags
//@ pp-record: stray.record
//@ diag: 10:2 error "#elif without #if"
//@ diag: 11:2 error "#else without #if"
//@ diag: 12:2 error "#endif without #if"
#elif 1
#else
#endif
int a;
