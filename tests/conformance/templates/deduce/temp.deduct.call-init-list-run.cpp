// [temp.deduct.call]/1: P = std::initializer_list<P'> deduces P' from
// each element of a braced-init-list argument; the deduced
// specialization receives the built initializer_list object.
//@ expect: run
//@ stdout: 6 2 3.5 99
#include "../../dcl.init.list/Inputs/initializer_list.h"
extern "C" int printf(const char*, ...);
template <class T> T sum(std::initializer_list<T> l) {
  T s = T();
  for (const T* p = l.begin(); p != l.end(); ++p) s += *p;
  return s;
}
template <class T> int count(const std::initializer_list<T>& l) { return int(l.size()); }
struct Acc {
  int v = 0;
  template <class T> Acc& add(std::initializer_list<T> l) {
    for (const T* p = l.begin(); p != l.end(); ++p) v += int(*p);
    return *this;
  }
};
int main() {
  Acc a;
  printf("%d %d %g %d\n", sum({1, 2, 3}), count({'x', 'y'}), sum({1.5, 2.0}),
         a.add({90, 4}).add({2L, 3L}).v);
}
