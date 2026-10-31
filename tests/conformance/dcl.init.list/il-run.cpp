// [dcl.init.list]/5-6: at run time the std::initializer_list object
// points at a backing array that lives as long as the object (or the
// reference) it initializes; initializer-list constructors receive it.
//@ expect: run
//@ stdout: 10 4 1 2 3 4
//@ stdout: 30 2 10
//@ stdout: 5 1
//@ stdout: 12 3 1
//@ stdout: 6 2 0
//@ stdout: 3.5
#include "Inputs/initializer_list.h"
extern "C" int printf(const char*, ...);
struct P { int a, b; };
int sum(std::initializer_list<int> l) {
  int s = 0;
  for (const int* p = l.begin(); p != l.end(); ++p) s += *p;
  return s;
}
struct V {
  int total, count, tag;
  V(std::initializer_list<int> l, int t = 1) : total(sum(l)), count(int(l.size())), tag(t) {}
  V(int a, int b) : total(a * b), count(2), tag(0) {}
};
V make() { return {1, 2, 3, 6}; }
template <class T> T tsum(std::initializer_list<T> l) {
  T s = 0;
  for (const T* p = l.begin(); p != l.end(); ++p) s += *p;
  return s;
}
int main() {
  std::initializer_list<int> l = {1, 2, 3, 4};
  printf("%d %d", sum(l), int(l.size()));
  for (const int* p = l.begin(); p != l.end(); ++p) printf(" %d", *p);
  printf("\n");
  const std::initializer_list<int>& r = {10, 20};
  printf("%d %d %d\n", sum(r), int(r.size()), *r.begin());
  std::initializer_list<P> ps = {{1, 2}, {3, 1}};
  printf("%d %d\n", ps.begin()->a * ps.begin()->b + ps.begin()[1].a, ps.begin()[1].b);
  V v = make();
  printf("%d %d %d\n", v.total, v.count - 1, v.tag);
  V w(2, 3);
  printf("%d %d %d\n", w.total, w.count, w.tag);
  printf("%g\n", tsum({1.5, 2.0}));
  return 0;
}
