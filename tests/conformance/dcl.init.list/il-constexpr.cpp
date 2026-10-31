// [dcl.init.list]/5: an object of type std::initializer_list<E> is built
// from a backing array of N const E, copy-initialized from the list's
// elements; constant evaluation reads it through begin()/end().
#include "Inputs/initializer_list.h"
struct P { int a, b; };
constexpr int sum(std::initializer_list<int> l) {
  int s = 0;
  for (const int* p = l.begin(); p != l.end(); ++p) s += *p;
  return s;
}
constexpr int dot(std::initializer_list<P> l) {
  int s = 0;
  for (const P* p = l.begin(); p != l.end(); ++p) s += p->a * p->b;
  return s;
}
static_assert(sum({1, 2, 3}) == 6);
static_assert(sum({42}) == 42);
static_assert(sum({}) == 0);
static_assert(dot({{1, 2}, {3, 4}}) == 14);
constexpr int local() {
  std::initializer_list<int> l = {4, 5, 6};
  return sum(l) + int(l.size());
}
static_assert(local() == 18);
constexpr int braced_local() {
  std::initializer_list<int> l{7, 8};
  return l.begin()[1] - l.begin()[0] + int(l.size());
}
static_assert(braced_local() == 3);
constexpr bool empty() {
  std::initializer_list<int> l = {};
  return l.size() == 0 && l.begin() == nullptr;
}
static_assert(empty());
