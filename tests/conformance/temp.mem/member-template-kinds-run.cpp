// [temp.mem], [temp.pre]/2: each kind of member template of a class
// template is redeclared in its specializations with its own head one
// level shallower: a class, a member function, a constructor, a
// conversion function, a static data member and an alias template (the
// negative sibling is member-template-kinds.cpp).
//@ expect: run
//@ std: c++20
//@ stdout: 3 5 7 2 4 8

#include <cstdio>

template <class T> struct A {
  template <class U> struct C { static constexpr int v = sizeof(U) + 2; };
  template <class U> int f(U u) const { return int(u) + t; }
  template <class U> A(U u) : t(int(u)) {}
  template <class U> operator U() const { return U(t + 1); }
  template <class U> static constexpr int s = sizeof(U) * 4;
  template <class U> using P = U[2];
  T t;
};

int main() {
  A<int> a(3);
  long l = a;
  typename A<int>::template P<int> p = {0, 0};
  std::printf("%d %d %d %d %d %d\n", A<int>::C<char>::v, a.f(2), a.f(4L),
              int(sizeof(p) / sizeof(p[0])), int(l), A<long>::s<short>);
}
