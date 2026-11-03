//@ args: -O1 -Xclang -disable-llvm-passes
// [stmt.iter], [intro.progress]/1: at exactly -O1 (clang `LoopInfoStack`
// without `UnrollLoops`) every loop's back-edge carries `!llvm.loop` with
// `llvm.loop.unroll.disable`, after `llvm.loop.mustprogress` where the
// loop may be assumed to terminate. A range-based for loop and a loop
// whose condition is a constant `true` get no `mustprogress`
// ([intro.progress]/1 trivially infinite loops); a non-loop statement
// (`do { } while (0)`) gets neither.
int g(int);
int f(int n, int (&a)[4]) {
  int s = 0;
  for (int i = 0; i < n; ++i) s += g(i);
  while (n > 0) { s += g(n); --n; }
  do { s += g(s); } while (s < 100);
  for (int& x : a) s += g(x);
  for (;;) { if (g(s)) break; }
  while (1) { if (g(s)) break; }
  do { s += g(1); } while (0);
  return s;
}
void spin() { while (true) {} }
