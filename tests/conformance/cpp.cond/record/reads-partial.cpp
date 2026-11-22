// [cpp.cond]: a condition cut off by an error before the end of its line
// keeps the reads made before the error and is marked partial (`X` is
// never read); so is an `#ifdef` without a name. An error found at the
// end of the line cuts nothing off: every name was read, and the list is
// not partial (spec/pp.md "Preprocessing record", condition reads).
//@ expect: compile-fail
//@ strict-diags
//@ pp-record: reads-partial.record
//@ diag: 13:7 error "token is not a valid binary operator in a preprocessor subexpression"
//@ diag: 15:13 error "expected value in expression"
//@ diag: 17:7 error "macro name missing"
#define V 1
#if V W X
#endif
#if V && W +
#endif
#ifdef
#endif
