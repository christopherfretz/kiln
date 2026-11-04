// [class.access.base]/2, [class.access]/3, [temp.expl.spec]: an explicit
// specialization's default base-class access and default member access
// follow the class-key of its own class-head, not the primary template's.
// Each explicit specialization below is defined with `struct` over a
// primary declared `class`, so its unannotated base is public and its
// members before any access-specifier are public: the base's public enum
// is reachable through the specialization (a numeric-library traits
// shape, `template <> struct traits<double> : traits<float> {}`), and the
// members are usable from outside. The key of the definition wins also
// when the specialization was first named (`X<int> *`) or declared with
// `class` before it is defined with `struct` (WP-611).
//@ expect: run
//@ stdout: 4 4 1 2 3
#include <cstdio>
template <class L>
class traits {
public:
  enum { Progress = 4 };
};
template <>
struct traits<double> : traits<float> {};

template <class L, int Arch>
class traits2 {
public:
  enum { Progress = 4 };
};
template <>
struct traits2<double, 1> : traits2<double, 0> {};
template <class L>
struct kernel {
  typedef traits2<L, 1> Traits;
  enum { Progress = Traits::Progress };
};

template <class T>
class X {
  int a;
};
X<int> *named_first;
template <>
struct X<int> {
  int a;
};

template <class T>
class Z {
  int a;
};
template <>
class Z<int>;
template <>
struct Z<int> {
  int a;
};

template <class T>
class Y {
  int a;
};
template <>
struct Y<char> {
  int a;
};

int main() {
  X<int> x;
  x.a = 1;
  Z<int> z;
  z.a = 2;
  Y<char> y;
  y.a = 3;
  std::printf("%d %d %d %d %d\n", traits<double>::Progress, kernel<double>::Progress, x.a, z.a,
              y.a);
  return traits<double>::Progress - 4;
}
