// Coverage of `do { ... } while (0)` macro bodies and statement
// expressions that end in a terminator (WP-1082, spec/codegen.md §5.4
// "Instrumentation"): clang 20+ propagates a `return` / `break` out of
// a do-while only when its exit count differs from the parent's, and a
// GNU statement expression no longer leaves its terminator to the
// enclosing statement (`VisitStmtExpr`); clang 19 adds a gap region
// after each.
//@ args: -fprofile-instr-generate -fcoverage-mapping -fcoverage-compilation-dir=/cov -Xclang -disable-llvm-passes
int ext(int);
#define CHECK(c)                                                               \
  do {                                                                         \
    if (!(c))                                                                  \
      return -1;                                                               \
  } while (0)
#define TRY(x) ({ if (ext(x)) return 1; ext(x + 1); })
int f(int a) {
  CHECK(a > 0);
  CHECK(a < 10);
  int v = TRY(a);
  do {
    if (a == 3)
      break;
    a = ext(a);
  } while (0);
  return v + a;
}
