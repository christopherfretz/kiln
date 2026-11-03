// WP-492: access groups of the instructions kiln inserts away from the
// insertion point in a parallel loop body (`#pragma clang loop
// vectorize(assume_safety)`), as clang's `LoopInfoStack::InsertHelper`
// sees them: one made by the function's builder at a moved insertion
// point gets the group, one made by `new X(.., InsertPt)` or appended
// directly does not. The `cleanup.dest` load of a cleanup's exit switch
// (`createLoadInstBefore`), the `cleanup.dest.slot` stores of branch
// fixups and the `cleanup.cond` flag stores before the outermost
// conditional (`setBeforeOutermostConditional`) have none.
// [cpp.pragma]/1, [stmt.iter], [stmt.jump]/2, [class.temporary]/4.
struct T { int v; ~T(); T(int); };
struct W { int a[4]; };
W mk(int);
int uset(const T &);
bool p(int);
__attribute__((optnone)) int f(int *a, int n) {
#pragma clang loop vectorize(assume_safety)
  for (int i = 0; i < n; ++i) {
    T t{a[i]};
    if (p(i)) break;
    if (p(i + 1)) continue;
    if (p(i + 2)) return 3;
    T u{i};
    a[i] = uset(t) + uset(u);
  }
  return 0;
}
__attribute__((optnone)) int g(int *a, int n, bool c) {
  int s = 0;
#pragma clang loop vectorize(assume_safety)
  for (int i = 0; i < n; ++i) {
    s += (c && p(i)) ? uset(T{a[i]}) : uset(T{i});
    try { s += uset(T{1}); } catch (...) { s = 0; }
  }
  return s;
}
__attribute__((optnone)) int h(int *a, int n) {
  int i = 0;
#pragma clang loop vectorize(assume_safety)
  while (i < n) {
    T t{a[i]};
    switch (a[i]) {
    case 0: { T q{1}; if (p(1)) goto out; break; }
    case 1: continue;
    default: a[i] = uset(T(2)) + (p(i) ? uset(T(3)) : 4);
    }
    ++i;
  }
out:
  return i;
}
__attribute__((optnone)) int k(int *a, int n) {
  int s = 0;
#pragma clang loop vectorize(assume_safety)
  for (int i = 0; i < n; ++i) {
    const W &w = mk(i);
    T *t = new T(a[i]);
    s += w.a[1] + ({ T z(i); uset(z); }) + (p(i) && uset(T(i)) ? 1 : 0);
    delete t;
  }
  return s;
}
