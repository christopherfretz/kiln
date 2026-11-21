// `-fsanitize-coverage=trace-pc-guard` with the program's own callbacks
// (no runtime): `__sanitizer_cov_trace_pc_guard_init` gets the module's
// guard array once, and every edge the program runs calls
// `__sanitizer_cov_trace_pc_guard`, except in a function the
// `-fsanitize-coverage-ignorelist=` names (`fun:`) or one that is
// `no_sanitize("coverage")`. The callbacks themselves (`__sanitizer_*`)
// are never instrumented. spec/codegen.md "Sanitizer coverage", WP-1863.
// clang 19's driver links the UBSan standalone runtime for any
// `-fsanitize-coverage=`, so the clang self-check lanes need it installed.
//@ expect: run
//@ requires: sanitizer-runtime
//@ args: -fsanitize-coverage=trace-pc-guard -fsanitize-coverage-ignorelist=%{test_dir}/Inputs/coverage-ignorelist.txt
//@ stdout: init 1
//@ stdout: listed 0
//@ stdout: marked 0
//@ stdout: counted 1
extern "C" int printf(const char *, ...);

static bool init_ok;
static bool armed;
static unsigned calls;

extern "C" void __sanitizer_cov_trace_pc_guard_init(unsigned *start, unsigned *stop) {
  init_ok = start != stop;
  for (unsigned *g = start; g != stop; ++g)
    *g = 1;
}

extern "C" void __sanitizer_cov_trace_pc_guard(unsigned *guard) {
  if (armed && *guard)
    ++calls;
}

// Named by the ignorelist.
extern "C" __attribute__((noinline)) int listed(int x) { return x > 2 ? x * 2 : x + 1; }

extern "C" __attribute__((noinline, no_sanitize("coverage"))) int marked(int x) {
  return x > 2 ? x * 3 : x + 2;
}

extern "C" __attribute__((noinline)) int counted(int x) { return x > 2 ? x * 4 : x + 3; }

template <class F> static unsigned probe(F f, int x) {
  calls = 0;
  armed = true;
  volatile int r = f(x);
  armed = false;
  (void)r;
  return calls;
}

int main() {
  unsigned a = probe(listed, 5);
  unsigned b = probe(marked, 5);
  unsigned c = probe(counted, 5);
  printf("init %d\nlisted %u\nmarked %u\ncounted %d\n", init_ok ? 1 : 0, a, b, c > 0 ? 1 : 0);
  return 0;
}
