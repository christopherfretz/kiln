// [lex.operators] P2996R13: at C++26 reflection is on by default
// (operator decision Q7, WP-920) and -fno-reflection turns it off: `^^`
// lexes as two `^` and `[:` as `[` `:`, as below C++26 (flag-off.cpp).
//@ args: -fno-reflection
//@ skip(clang): -fno-reflection is kiln's flag; clang has no P2996
//@ expect: compile-fail
//@ diag: 11 error "expected expression"
//@ diag: 12 error "expected expression"
//@ diag: 13 error "expected expression"
//@ diag: 14:10 error "expected variable name or 'this' in lambda capture list"
int a = ^^int;
int b = 1 ^^ 2;
int c = 1 ^ ^int;
int d = [: 1 :];
