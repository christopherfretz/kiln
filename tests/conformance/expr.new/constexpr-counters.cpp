// [expr.new], [expr.delete] + kiln Oven counters (spec/oven.md "Counters"):
// `allocs` counts new-expressions, and `peak_bytes`/`peak_arena` include the
// heap. The third array reuses the block the first one freed, so the heap
// never holds more than two arrays.
//@ oven: 17 allocs<=3 peak_bytes<=2048 peak_arena<=2048
constexpr int f() {
  int *a = new int[256]();
  int *b = new int[256]();
  delete[] a;
  int *c = new int[256]();
  int r = b[255] + c[255];
  delete[] b;
  delete[] c;
  return r;
}
constexpr int g() { return f() + 1; }
static_assert(g() == 1);
