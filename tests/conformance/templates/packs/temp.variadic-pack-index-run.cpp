// [expr.prim.pack.index]: pack indexing at run time, with a loop over the
// pack through an index sequence.
//@ expect: run
template <class... Ts> int get(int i, Ts... ts) {
  int r = -1;
  int k = 0;
  ((r = (k++ == i ? ts : r)), ...);
  return r;
}
template <class... Ts> int third(Ts... ts) { return ts...[2]; }
int main() {
  if (third(5, 6, 7, 8) != 7) return 1;
  if (get(1, 10, 20, 30) != 20) return 2;
  return 0;
}
