// Coverage branch regions of constant-folded conditions (WP-1082,
// spec/codegen.md §5.4 "Instrumentation"): clang 20+ zeroes only the
// side that can never be taken (`createBranchRegion` with a folded
// condition), so the taken side keeps its counter; clang 19 zeroes both.
// It also compares counters by simplification (`IsCounterEqual`) when
// deciding whether a loop or `if` exit needs a new region.
//@ args: -fprofile-instr-generate -fcoverage-mapping -fcoverage-compilation-dir=/cov -Xclang -disable-llvm-passes
int ext(int);
int f(int x) {
  int r = 0;
  if (1)
    r = ext(1);
  if (0)
    r = ext(2);
  while (x > 0 && 1)
    x = ext(x);
  for (; 0;)
    r++;
  if (true || x)
    r += 3;
  r += (0 && x) ? 1 : 2;
  do {
    r += ext(r);
  } while (false);
  return r;
}
