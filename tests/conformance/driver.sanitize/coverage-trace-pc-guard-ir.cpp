// `-fsanitize-coverage=trace-pc-guard,trace-cmp` (frontend output): a
// `no_sanitize("coverage")` function is `nosanitize_coverage`, which
// `SanitizerCoverage` leaves alone; the others are untouched until the
// pass runs (crates/kiln_driver/tests/sanitize_coverage.rs compares the
// instrumentation). spec/codegen.md "Sanitizer coverage",
// spec/driver.md "Sanitizers", WP-1863.
//@ expect: ir
//@ args: -fsanitize-coverage=trace-pc-guard,trace-cmp -Xclang -disable-llvm-passes
extern "C" int g(int);

extern "C" int f(int x, int y) {
  if (x > y)
    return g(x);
  switch (y) {
  case 1:
    return 3;
  case 7:
    return 4;
  }
  return x;
}

extern "C" __attribute__((no_sanitize("coverage"))) int skipped(int x) {
  return x ? g(x) : 1;
}
