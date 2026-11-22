// [cpp.cond]/1: a conditional the file ends inside is an error. The opener
// is flagged unterminated and the last group runs to the end of the file
// (spec/pp.md "Preprocessing record", conditionals).
//@ expect: compile-fail
//@ strict-diags
//@ pp-record: unterminated.record
//@ diag: 8:2 error "unterminated conditional directive"
#if 1
int a;
#else
int b;
