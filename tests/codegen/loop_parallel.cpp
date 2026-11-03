// [cpp.pragma]/1, [stmt.iter]: `#pragma clang loop vectorize(assume_safety)`
// and `interleave(assume_safety)` make the loop parallel (clang
// `LoopAttributes::IsParallel`): every instruction emitted while it is
// active that may access memory (loads, stores, atomics and calls whose
// callee is not `memory(none)`) gets its `distinct !{}` access group as
// `!llvm.access.group`, and its `!llvm.loop` id lists the group under
// `llvm.loop.parallel_accesses`. Under nested parallel loops the access
// group is the list of every active loop's group, outermost first. The
// loop is active from where clang pushes it: before the condition of a
// `for` (and range-based `for`), after the condition of a `while`, and
// only for the back-edge of a `do`.
struct S { int x; ~S(); };
int c(int);
__attribute__((const)) int k(int);
void f(int *a, int n) {
#pragma clang loop vectorize(assume_safety)
  for (int i = 0; i < n; ++i) a[i] = a[i] + 1;
}
void nested(int *a, int *b, int n) {
#pragma clang loop vectorize(assume_safety)
  for (int i = 0; i < n; ++i) {
    S s{a[i]};
#pragma clang loop interleave(assume_safety)
    for (int j = 0; j < n; ++j) b[j] += s.x;
#pragma unroll 2
    for (int j = 0; j < n; ++j) b[j] -= 1;
    a[i] = 0;
  }
}
void w(int *a, int n) {
  int i = 0;
#pragma clang loop vectorize(assume_safety)
  while (i < n) { a[i] = 1; ++i; }
#pragma clang loop interleave(assume_safety) vectorize_width(4)
  do { a[i] = 2; --i; } while (i > 0);
}
// A destructor and a `pure`-less call get the group; a `const` callee and a
// `memory(none)` intrinsic do not.
void calls(int *a, int n) {
#pragma clang loop vectorize(assume_safety)
  for (S s{1}; s.x < n; ++s.x) a[s.x] = c(s.x) + k(s.x) + __builtin_popcount(s.x);
}
void ranged(int (&a)[4], int *p) {
#pragma clang loop vectorize(assume_safety)
  for (int &x : a) { x = 0; __atomic_fetch_add(p, 1, __ATOMIC_SEQ_CST); }
}
