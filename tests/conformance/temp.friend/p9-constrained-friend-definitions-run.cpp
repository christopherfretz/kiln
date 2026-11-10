// [temp.friend]/9: a non-template friend with a requires-clause, and a
// friend function template with a constraint that depends on an enclosing
// template parameter, are valid as definitions (a body, `= default` or
// `= delete`). Each specialization of the enclosing class template
// declares its own such friend, so instantiating A<char>, A<short> and
// A<int> neither redefines one nor redeclares the namespace scope `f`,
// `g` or `h` (which are distinct functions, found only by ordinary
// lookup).
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 4 | 11 12 14 | 101 102 104 | 1 1 1 | 7 8 9
#include <cstdio>

template <class T> concept C = sizeof(T) > 0;
template <class T, class U> concept D = sizeof(T) + sizeof(U) > 1;

int f(int) { return 7; }
template <class U> int g(U) { return 8; }
int h() { return 9; }

template <class T> struct A {
  int v = sizeof(T);
  friend int f(A a) requires C<T> { return a.v; }
  template <class U> friend int g(A a, U u) requires C<T> { return a.v + u; }
  template <D<T> U> friend int k(A a, U u) { return a.v + u; }
  friend int m(A a, auto u) requires C<T> { return a.v + u; }
  friend bool operator==(const A&, const A&) requires C<T> = default;
  friend void del(A, long) requires C<T> = delete;
  friend int h(A) requires true { return 1; }
};

int main() {
  A<char> c;
  A<short> s;
  A<int> i;
  std::printf("%d %d %d | ", f(c), f(s), f(i));
  std::printf("%d %d %d | ", g(c, 10), g(s, 10), g(i, 10));
  std::printf("%d %d %d | ", k(c, 100), k(s, 100), m(i, 100));
  std::printf("%d %d %d | ", int(c == c), int(s == s), h(i));
  std::printf("%d %d %d\n", f(0), g(0), h());
}
