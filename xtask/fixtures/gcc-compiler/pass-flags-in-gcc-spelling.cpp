//@ expect: compile-ok
//@ args: --gcc-toolchain=/no/such/prefix -fclang-compat=19 -ferror-limit=5
//@ args: -fconstexpr-steps=1000 -fkiln-source-path-style=gcc -resource-dir /r
// GCC gets none of the kiln/clang-only flags (`-ferror-limit=` and
// `-fconstexpr-steps=` in GCC's spelling).
int f() { return 1; }
