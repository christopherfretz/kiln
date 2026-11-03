// WP-492: `loop_parallel_inserts.cpp` above -O0. The `llvm.lifetime.start`
// of a full-expression temporary bound in a conditional branch is made
// by the builder moved to the outermost conditional's starting block
// (clang `EmitMaterializeTemporaryExpr`), so it gets the active access
// group; the matching end and the cleanups' flag stores keep the plain
// rules. [cpp.pragma]/1, [class.temporary]/4.
//@ args: -O1
struct S { int v; };
int use(const S &);
struct T { int v; ~T(); };
int uset(const T &);
bool p(int);
__attribute__((optnone)) int f(int *a, int n, bool c) {
#pragma clang loop vectorize(assume_safety)
  for (int i = 0; i < n; ++i)
    a[i] = c ? use(S{a[i]}) : 0;
  return 0;
}
__attribute__((optnone)) int g(int *a, int n, bool c) {
#pragma clang loop vectorize(assume_safety)
  for (int i = 0; i < n; ++i)
    a[i] = c ? uset(T{a[i]}) : 0;
  return 0;
}
__attribute__((optnone)) int h(int *a, int n) {
#pragma clang loop vectorize(assume_safety)
  for (int i = 0; i < n; ++i) {
    T t{a[i]};
    if (p(i)) break;
    if (p(i + 1)) continue;
    if (p(i + 2)) return 3;
    a[i] = uset(t) + (p(i) ? use(S{i}) : 1);
  }
  return 0;
}
