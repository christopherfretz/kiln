// C89 3.1.9: under -E strict C89 lexes no line comments, even in a
// skipped group, where an unmatched quote after the slashes is not lexed
// as an error, in a macro body, where the slashes are replacement
// tokens, and after a directive, where they are extra tokens. The
// -pedantic "not allowed" warning is for compiles only.
//@ std: c89
//@ args: -pedantic
//@ skip(gcc): GCC has no line comments in C89, and this header is one
//@ expect: pp
//@ strict-diags
//@ diag: 16:8 warning "extra tokens at end of #endif directive"
#if 0
// it's skipped "
#else
e // f
#endif // l
#define M(x) x // g
M(h) // i
