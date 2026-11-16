// [dcl.constexpr]/3 (CWG2552): only the definition of a variable of static
// or thread storage duration ends a constant evaluation; a block-scope
// `extern` declaration defines nothing. Found running GCC's C++ testsuite
// (WP-1468).
//@ std: c++23
//@ expect: compile-ok
constexpr bool f() { extern thread_local int t; extern int s; return true; }
static_assert(f());
constexpr int g(int x) { extern int s; return x + 1; }
static_assert(g(1) == 2);
