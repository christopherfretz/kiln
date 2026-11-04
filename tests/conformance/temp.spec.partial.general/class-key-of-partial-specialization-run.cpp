// [class.access.base]/2, [class.access]/3, [temp.spec.partial.general]: a
// partial specialization's default base-class access and default member
// access follow the class-key of its own class-head, not the primary
// template's, and a specialization implicitly instantiated from it takes
// that key. Each partial specialization below is defined with `struct`
// over a primary declared `class` (one of them first declared `class`
// itself), so its unannotated base and its leading members are public
// (WP-611).
//@ expect: run
//@ stdout: 4 4 1 2
#include <cstdio>
template <class L>
class traits {
public:
  enum { Progress = 4 };
};
template <class L>
struct traits<L *> : traits<L> {};
template <class L>
struct traits<L &> : traits<L *> {};

template <class T>
class X {
  int a;
};
template <class T>
struct X<T *> {
  int a;
};

template <class T>
class W;
template <class T>
class W<T *>;
template <class T>
struct W<T *> {
  int a;
};

int main() {
  X<int *> x;
  x.a = 1;
  W<int *> w;
  w.a = 2;
  std::printf("%d %d %d %d\n", traits<int *>::Progress, traits<int &>::Progress, x.a, w.a);
  return traits<int *>::Progress - 4;
}
