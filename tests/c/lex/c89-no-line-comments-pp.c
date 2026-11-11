// C89 3.1.9: in preprocessed output clang does not lex `//` as a
// comment in strict C89 at all, so these lines and the next are tokens.
//@ std: c89
//@ skip(gcc): GCC has no line comments in C89, and this header is one
//@ expect: pp
a //**/ b
c // d __STDC__
