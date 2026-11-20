// [class.friend]/2, /3, [temp.friend]: every kind of friend a class
// template declares is instantiated with it: a function, a function
// defined in the class, a member function, a constructor and a
// conversion function of another class, a function template and a class
// template (the negative sibling is friend-kinds-in-class-template.cpp).
//@ expect: run
//@ std: c++20
//@ stdout: 1 2 3 4 5 6 7

#include <cstdio>

template <class T> struct A;
int f(const A<int> &);
template <class U> int ft(const U &);
template <class U> struct CT;
struct M {
  int m(const A<int> &);
  int c = 0;
  M();
  explicit M(const A<int> &);
  operator int();
};

template <class T> class A {
  T v = 1;
  friend int f(const A &);
  friend int g(const A &a) { return a.v + 1; }
  friend int M::m(const A<int> &);
  friend M::M(const A<int> &);
  friend M::operator int();
  template <class U> friend int ft(const U &);
  template <class U> friend struct CT;
};

int f(const A<int> &a) { return a.v; }
int M::m(const A<int> &a) { return a.v + 2; }
M::M() = default;
M::M(const A<int> &a) : c(a.v + 3) {}
M::operator int() { return A<int>().v + 4; }
template <class U> int ft(const U &a) { return a.v + 5; }
template <class U> struct CT { static int get(const A<U> &a) { return a.v + 6; } };

int main() {
  A<int> a;
  std::printf("%d %d %d %d %d %d %d\n", f(a), g(a), M().m(a), M(a).c, int(M()), ft(a),
              CT<int>::get(a));
}
