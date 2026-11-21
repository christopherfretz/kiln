// Which `__attribute__` arguments are parsed as an unknown attribute's
// (WP-1853): clang's parser, not kiln's own argument tables, decides
// (clang `ParseAttributeArgsCommon`, `ParsedAttr::UnknownAttribute`).
// `nodiscard` has no GNU spelling, so a lone identifier is an identifier
// argument and only "unknown attribute" follows, although kiln parses
// `[[nodiscard(...)]]`'s argument as an expression; clang's string
// literal table still holds by name. An attribute of another target
// (`long_call`, `signal`, `dllimport`) is known to clang's parser: its
// arguments are expressions, an invalid one is an error that drops it.
// The release-dependent cases are in the `-clang19` / `-clang22`
// siblings. spec/sema.md §4.5.
//@ verify
//@ args: --target=aarch64-unknown-linux-gnu
int v;
__attribute__((nodiscard(undeclared))) int g1(); // expected-warning {{unknown attribute 'nodiscard' ignored}}
__attribute__((__nodiscard__(undeclared))) int g2(); // expected-warning-re {{unknown attribute '{{(__)?}}nodiscard{{(__)?}}' ignored}}
__attribute__((nodiscard(undeclared + 1))) int g3(); // expected-error {{expected string literal as argument of 'nodiscard' attribute}}
__attribute__((nodiscard("why"))) int g4(); // expected-warning {{unknown attribute 'nodiscard' ignored}}
__attribute__((long_call(undeclared))) void h1(); // expected-error {{use of undeclared identifier 'undeclared'}}
__attribute__((signal(undeclared))) void h2(); // expected-error {{use of undeclared identifier 'undeclared'}}
__attribute__((dllimport(undeclared))) void h3(); // expected-error {{use of undeclared identifier 'undeclared'}}
__attribute__((__long_call__(undeclared))) void h4(); // expected-error {{use of undeclared identifier 'undeclared'}}
__attribute__((mips16)) void h5(); // expected-warning {{unknown attribute 'mips16' ignored}}
__attribute__((long_call(v))) void h6(); // expected-warning {{unknown attribute 'long_call' ignored}}
__attribute__((foo_bar(undeclared, 1))) void h7(); // expected-warning {{unknown attribute 'foo_bar' ignored}}
