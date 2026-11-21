// `-fsanitize=fuzzer-no-link` (frontend output) keeps the library calls
// libFuzzer intercepts: definitions and call sites carry
// `"no-builtin-<name>"` for the seven of them clang knows as builtins
// (`bcmp`, `memcmp`, `strcmp`, `strncmp`, `strcasecmp`, `strncasecmp`,
// `strstr`). Its coverage set (crates/kiln_driver/tests/
// sanitize_coverage.rs) is instrumented after the frontend.
// spec/codegen.md "Sanitizer coverage", WP-1863.
//@ expect: ir
//@ args: -fsanitize=fuzzer-no-link -fno-sanitize-coverage=trace-cmp -Xclang -disable-llvm-passes
extern "C" int g(int);

extern "C" int f(int x, int y, void (*q)()) {
  if (x > y) {
    q();
    return g(x);
  }
  return y;
}
