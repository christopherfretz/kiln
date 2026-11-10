//@ expect: compile-ok
//@ args: -Weverything -Wno-c++98-compat -Werror=invalid-static-assert-message
//@ args: -Wall -Werror=unused-variable -Rsearch-path-usage
// The clang-only diagnostic flags are dropped for GCC (`-Weverything`,
// `-R<remark>`, a `-W`/`-Werror=` name GCC does not list); the ones GCC
// knows reach it (WP-1261).
int f() { return 1; }
