// [lex.operators] P2996R13: `^^` is one token only in reflection mode.
// Reflection is on by default only at C++26 (operator decision Q7,
// WP-920); below it, without -freflection, `^^` lexes as two `^` and
// `^^int` is a parse error under kiln and clang alike; `[:` is `[` `:` (a
// lambda introducer). C++26 with -fno-reflection: flag-off-26.cpp.
//@ std: c++23
//@ expect: compile-fail
//@ diag: 12 error "expected expression"
//@ diag: 13 error "expected expression"
//@ diag: 14 error "expected expression"
//@ diag: 15:10 error "expected variable name or 'this' in lambda capture list"
int a = ^^int;
int b = 1 ^^ 2;
int c = 1 ^ ^int;
int d = [: 1 :];
