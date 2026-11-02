// [lex.operators] P2996R13: `^^` is one token only in reflection mode.
// Without -freflection it lexes as two `^` and `^^int` is a parse error
// under kiln and clang alike; `[:` is `[` `:` (a lambda introducer).
//@ expect: compile-fail
//@ diag: 9 error "expected expression"
//@ diag: 10 error "expected expression"
//@ diag: 11 error "expected expression"
//@ diag: 12:10 error "expected variable name or 'this' in lambda capture list"
int a = ^^int;
int b = 1 ^^ 2;
int c = 1 ^ ^int;
int d = [: 1 :];
