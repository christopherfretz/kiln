// GNU `__restrict` method qualifier (WP-1091): clang's nested-name drops
// it (`_ZN1T2okEv`, `_ZNK1T1cEv`, `_ZNR1T1rEv`), a pointer-to-member
// function type keeps it (`M1TrFivE`, `M1TrKFivE`).
struct T {
  int x = 1;
  int ok() __restrict { return x; }
  int c() const __restrict { return x; }
  int r() __restrict & { return x; }
  int out() volatile __restrict;
};
int T::out() volatile __restrict { return x; }
int pm(int (T::*m)() __restrict, T& t) { return (t.*m)(); }
int pm2(int (T::*m)() const __restrict, const T& t) { return (t.*m)(); }
int use(T& t, const T& ct) {
  return t.ok() + ct.c() + t.r() + t.out() + pm(&T::ok, t) + pm2(&T::c, ct);
}
