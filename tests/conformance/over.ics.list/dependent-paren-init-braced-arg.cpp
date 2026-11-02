// [over.ics.list], [dcl.init.general]/16.6.2.1: in a function template,
// a direct-initializer `C c({1, 2}, a)` (or `new C({1, 2}, a)`) whose
// other argument is type-dependent keeps the braced-init-list argument
// across instantiation, and the selected constructor list-initializes
// its `std::initializer_list` parameter from it (libc++
// `unordered_multiset(initializer_list, size_type, const allocator_type&)`
// from a test templated on the allocator). Verified against clang 19.
//@ expect: run
#include <initializer_list>
struct C {
  int sum = 0;
  C(std::initializer_list<int> il, int k) {
    for (int x : il) sum += x;
    sum *= k;
  }
};
template <class T> int direct(T a) {
  C c({1, 2, 3}, a);
  return c.sum;
}
template <class T> int heap(T a) {
  C* p = new C({1, 2}, a);
  int s = p->sum;
  delete p;
  return s;
}
template <class T> constexpr int sz(T a) {
  struct D {
    int n;
    constexpr D(std::initializer_list<T> il, T) : n(int(il.size())) {}
  } d({a, a, a}, a);
  return d.n;
}
static_assert(sz(1) == 3);
int main() { return direct(2) == 12 && heap(3) == 9 ? 0 : 1; }
