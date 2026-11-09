// Coverage mapping of `&&` / `||` (WP-1082, spec/codegen.md §5.4
// "Instrumentation"): clang 20+ (`-fclang-compat=20` and later) maps the
// gap between a logical operator's operands, counted as the right
// operand runs; clang 19 has no region there. Operands on one line, on
// several lines, from a macro, with a comment between them, nested.
//@ args: -fprofile-instr-generate -fcoverage-mapping -fcoverage-compilation-dir=/cov -Xclang -disable-llvm-passes
#define BOTH(a, b) ((a) && (b))
int f(int a, int b, int c) {
  int r = 0;
  if (a && b)
    r = 1;
  if (a ||
      b)
    r += 2;
  if (BOTH(a, c))
    r += 3;
  bool t = a /* first */ && /* second */ b;
  return r + (a && (b || c)) + t;
}
