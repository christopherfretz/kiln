// [expr.prim.lambda.capture]/17: init-capture packs at run time, mixed with
// a plain init-capture and captured by reference.
//@ expect: run
//@ stdout: 9 0 13
extern "C" int printf(const char *, ...);
template <class... T> int byref(T... ts) {
  [&...xs = ts] { ((xs += 1), ...); }();
  return (ts + ... + 0);
}
template <class... T> int run(T... ts) {
  return [...xs = ts, n = sizeof...(ts)] { return (xs + ... + 0) + int(n); }();
}
int main() { printf("%d %d %d\n", run(1, 2, 3), run(), byref(5, 6)); return run(1, 2) != 5; }
