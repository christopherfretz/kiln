// [lex.phases]/1.1: source bytes that are not valid UTF-8 outside a comment or
// literal are an error, as in clang. The bytes are dropped: an identifier ends
// there, and no "unexpected character" error follows.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:11 error "source file is not valid UTF-8"
//@ diag: 10:6 error "source file is not valid UTF-8"
//@ diag: 10:6 error "expected ';' after top level declarator"
int x = 1 é + 2;
int aéb = 0;
