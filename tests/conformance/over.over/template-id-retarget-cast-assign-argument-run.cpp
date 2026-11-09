// [over.over]/1, /3, [expr.static.cast]/4, [expr.cast]/4,
// [expr.type.conv]/1, [expr.assign]/3, [over.best.ics]/1: a function
// template-id whose explicit arguments alone complete one specialization
// (`f<int>`, `U` defaulted to `void`) is still an overload set when a
// target type is known: a `static_cast`, a C-style or functional cast, an
// assignment and an argument passed to a pointer parameter (one
// candidate, an overloaded call, an explicit specialization's parameter)
// all select the specialization the target deduces (`U = int`), not the
// one the explicit arguments complete. A cast must not reinterpret the
// early one (WP-1142: kiln did, or rejected the program).
//@ expect: run
//@ stdout: 4 4 4 4 4 4 0 | 4 4 4 4 4 4
#include <cstdio>
template <class U> struct Sz { static constexpr int v = sizeof(U); };
template <> struct Sz<void> { static constexpr int v = 0; };
template <class T, class U = void> int f(U *, T) { return Sz<U>::v; }
using FP = int (*)(int *, int);
using FR = int (&)(int *, int);
int take(FP g) { return g(0, 0); }
int take2(FP g) { return g(0, 0); }
int take2(double) { return -1; }
template <class F> int tt(F g) { return g(0, 0); }
int main() {
  FP a = (FP)f<int>;
  FP b = FP(f<int>);
  FP c = static_cast<FP>(f<int>);
  FP d = (FP)&f<int>;
  FR e = static_cast<FR>(f<int>);
  FP g;
  g = f<int>;
  int (*h)(void *, int) = (int (*)(void *, int))f<int>;
  std::printf("%d %d %d %d %d %d %d | ", a(0, 0), b(0, 0), c(0, 0), d(0, 0), e(0, 0), g(0, 0),
              h(0, 0));
  FP i;
  i = &f<int>;
  std::printf("%d %d %d %d %d %d\n", i(0, 0), take(f<int>), ::take(&f<int>), take2(f<int>),
              tt<FP>(f<int>), take(static_cast<FP>(f<int>)));
}
