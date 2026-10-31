// [over.match.list]/1.1: list-initialization of a class with an
// initializer-list constructor tries it first, with the whole list as
// its argument; the remaining parameters take their defaults.
#include "Inputs/initializer_list.h"
struct V {
  int how, sum, extra;
  constexpr V(std::initializer_list<int> l, int k = 100) : how(1), sum(0), extra(k) {
    for (const int* p = l.begin(); p != l.end(); ++p) sum += *p;
  }
  constexpr V(int a, int b) : how(2), sum(a + b), extra(0) {}
};
constexpr V a{1, 2};
static_assert(a.how == 1 && a.sum == 3 && a.extra == 100);
constexpr V b = {3, 4, 5};
static_assert(b.how == 1 && b.sum == 12);
constexpr V c(1, 2);
static_assert(c.how == 2 && c.sum == 3);
constexpr V d({1, 2}, 7);
static_assert(d.how == 1 && d.sum == 3 && d.extra == 7);
constexpr V e = V{9};
static_assert(e.how == 1 && e.sum == 9);
constexpr V make() { return {4, 4}; }
static_assert(make().how == 1 && make().sum == 8);
constexpr int take(V v) { return v.how * 10 + v.sum; }
static_assert(take({2, 3}) == 15);
// The parameter may be a reference to const std::initializer_list.
struct W {
  double s = 0;
  constexpr W(const std::initializer_list<double>& l) {
    for (const double* p = l.begin(); p != l.end(); ++p) s += *p;
  }
};
static_assert(W{1.5, 2.5}.s == 4.0);
