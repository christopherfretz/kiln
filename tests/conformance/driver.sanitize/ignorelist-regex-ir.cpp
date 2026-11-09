// A `#!special-case-list-v1` ignorelist is regular expressions (LLVM
// `SpecialCaseList`, version 1, at both `-fclang-compat=` levels): each
// pattern and section name is a POSIX extended regex anchored at both
// ends, with every `*` read as `.*`. `[undef]` names no sanitizer, so its
// `fun:.*` drops nothing; `skip_a`, `omit_b` and `star_x` lose both
// checks; `only_shi` loses its shift check only; `skip_c` and `kept` keep
// both. spec/driver.md "Sanitizers", WP-1043.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=signed-integer-overflow,shift -fsanitize-ignorelist=%{test_dir}/Inputs/ignorelist-regex.txt

int skip_a(int a, int b) { return (a + b) << b; }
int omit_b(int a, int b) { return (a + b) << b; }
int skip_c(int a, int b) { return (a + b) << b; }
int star_x(int a, int b) { return (a + b) << b; }
int only_shi(int a, int b) { return (a + b) << b; }
int kept(int a, int b) { return (a + b) << b; }
