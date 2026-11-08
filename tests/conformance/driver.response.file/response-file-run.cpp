// An `@file` argument is replaced by the arguments in the file (GNU
// tokenizer: quotes and backslashes), and a nested `@file` is expanded
// relative to the current directory, as clang's driver does. CMake links
// through `@objects1.rsp` (z3). spec/driver.md "Response files". WP-799.
//@ expect: run
//@ strict-diags
//@ args: @tests/conformance/driver.response.file/Inputs/defines.rsp
//@ stdout: hi there 3 4
extern "C" int printf(const char *, ...);
int main() { printf("%s %d %d\n", GREETING, N, M); }
