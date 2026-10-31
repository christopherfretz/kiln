// [expr.const] (clang extension __builtin_constant_p), kiln's trial
// evaluation (spec/oven.md "Trial evaluation"): a failed trial's writes to
// heap objects that existed before it are rolled back, nested or not, and
// failed trials hold no memory afterwards (O(1) in their number). clang
// keeps the writes of a failed speculative evaluation (it yields 70 below).
//@ skip(clang): clang keeps the heap writes of a failed speculative evaluation
//@ oven: peak_arena<=512 allocs<=3
constexpr int set_fail(int* p, int v) { *p = v; int z = 0; return 1 / z; }
constexpr int set(int* p, int v) { *p = v; return 1; }

constexpr int heap_rollback() {
  int* h = new int[2]{1, 2};
  int r = __builtin_constant_p(set_fail(h, 7));
  int v = h[0] * 10 + h[1];
  delete[] h;
  return v * 10 + r;
}
static_assert(heap_rollback() == 120);

// An inner successful trial's write is undone with the outer trial.
constexpr int inner_then_fail(int* h) {
  int a = __builtin_constant_p(set(h, 5));
  int z = 0;
  return a / z;
}
constexpr int nested_rollback() {
  int* h = new int(1);
  int r = __builtin_constant_p(inner_then_fail(h));
  int v = *h;
  delete h;
  return v * 10 + r;
}
static_assert(nested_rollback() == 10);

// Many failed trials over one heap object: the undo log is released after
// each, so memory does not grow with their number.
constexpr int many_trials(int n) {
  int* h = new int[64]{};
  int r = 0;
  for (int i = 0; i < n; ++i) {
    r += __builtin_constant_p(set_fail(h + i % 64, i));
  }
  int s = 0;
  for (int i = 0; i < 64; ++i)
    s += h[i];
  delete[] h;
  return r + s;
}
static_assert(many_trials(10000) == 0);
