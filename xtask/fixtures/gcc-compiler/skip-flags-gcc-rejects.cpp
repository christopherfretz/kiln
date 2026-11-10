//@ expect: compile-ok
//@ args: -fsanitize-ignorelist=%{test_dir}/README.md
// A flag GCC has no equivalent for, and that the harness cannot drop
// without changing what the test means, skips the test with GCC's own
// complaint (the flag probe, WP-1261) instead of failing it.
int f() { return 1; }
