// [dcl.init.list]/5, [dcl.init.aggr]/4 (WP-1008): an array initializer
// clang copies from a constant (`EmitArrayInit`'s `constinit` global, the
// `IsConstantAggregate` local) still makes a fresh, modifiable object each
// time it is evaluated, and an element with a side effect or a run-time
// value is never folded.
//@ expect: run
//@ args: -O0
//@ stdout: 29 11 75 | 1 0 100 | 9 2 3 4 5 | 1 2 3 4 5 | 1 2 3 4 1 | 2 | 1 2 3 4 7 | 8 2 | 2 2
#include <cstdio>
#include <initializer_list>

struct tm_ {
  int sec, min, hour, mday, mon, year;
  long off;
  const char* zone;
};
constexpr tm_ make_tm(int year, int mon, int mday) {
  auto t = tm_();
  t.mday = mday;
  t.mon = mon - 1;
  t.year = year - 1900;
  return t;
}
struct S {
  int x;
  constexpr S(int v = 7) : x(v) {}
};
constexpr int ice() { return __builtin_is_constant_evaluated() ? 1 : 2; }
int g;

void print5(const S* s) {
  std::printf("%d %d %d %d %d", s[0].x, s[1].x, s[2].x, s[3].x, s[4].x);
}

int main() {
  // The backing array of an initializer list, from constexpr calls.
  for (const tm_& t : {make_tm(1975, 12, 29), make_tm(2000, 1, 1)})
    std::printf("%d %d %d | ", t.mday, t.mon, t.year);
  // A mutable local initialized from the constant, modified later; the
  // next evaluation starts from the constant again.
  for (int i = 0; i < 2; ++i) {
    S s[5] = {1, 2, 3, 4, 5};
    if (i == 0) {
      s[0].x = 9;
      print5(s);
      std::printf(" | ");
    } else {
      print5(s);
    }
  }
  std::printf(" | ");
  // Side effects stay: `g++` runs once per evaluation.
  for (int i = 0; i < 2; ++i) {
    S b[5] = {1, 2, 3, 4, g++};
    if (i == 1) print5(b);
  }
  std::printf(" | %d | ", g);
  // A filler element and a member array inside a non-constant aggregate.
  struct {
    S a[8];
    int b;
  } t = {{1, 2, 3, 4}, g};
  std::printf("%d %d %d %d %d | %d %d", t.a[0].x, t.a[1].x, t.a[2].x, t.a[3].x, t.a[4].x,
              t.a[7].x + 1, t.b);
  // A trial fold: `__builtin_is_constant_evaluated()` is false at run time.
  S c[5] = {ice(), ice(), 3, 4, 5};
  std::printf(" | %d %d\n", c[0].x, c[1].x);
}
