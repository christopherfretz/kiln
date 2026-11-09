// [module.interface], [basic.def.odr]/14-15, spec/modules.md §11.5: a
// named module compiled separately and imported by two translation
// units. The non-inline exported function, variable and member defined
// only in the interface link exactly once (from ts's object); the
// importers instantiate the exported templates themselves and share the
// one exported inline variable.
//@ expect: run
//@ std: c++20
//@ module: ts=Inputs/ts.cppm
//@ unit: Inputs/ts-user.cpp
//@ stdout: 42 4 7 2.5 3 1 | 43 4 3 3 1 | 11 1 calls 3 55
import ts;
extern "C" int printf(const char *, ...);
int ts_user();
int *ts_shared_addr();

int main() {
  Counter c;
  Box<double> b{2.5};
  printf("%d %d %d %g %d %d | ", ts_answer(), ts_twice(2), ts_sum(3, 4), b.get(),
         Box<int>{3}.get(), c.bump());
  printf("%d %d %d %d %d | ", ts_answer(), ts_twice(2), ts_sum(1, 2), Box<int>{3}.get(),
         Counter{}.bump());
  int u = ts_user();
  printf("%d %d calls %d %d\n", ts_shared, &ts_shared == ts_shared_addr(), ts_calls, u);
}
