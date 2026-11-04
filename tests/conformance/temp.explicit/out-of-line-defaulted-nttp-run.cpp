// [temp.param]/12, [temp.over.link]/6, [temp.mem]: an out-of-line
// definition of a member function template whose template parameter list
// leaves out the defaults of the declaration (a defaulted non-type
// parameter, here an `enable_if`-constrained one, and a defaulted type
// parameter) matches the declaration, defaults merged; two constructor
// templates differing only in the constraint are distinct, each matched
// by its own definition. No explicit instantiation. WP-580.
//@ expect: run
//@ strict-diags
//@ stdout: 2 1 2 5 4
#include <cstdio>
#include <string>
#include <type_traits>
struct Cord {
  template <typename T, std::enable_if_t<std::is_same<T, std::string>::value, int> = 0>
  explicit Cord(T &&src);
  int n = 0;
};
template <typename T, std::enable_if_t<std::is_same<T, std::string>::value, int>>
Cord::Cord(T &&src) : n((int)src.size()) {}

template <class C> struct S {
  template <class T, std::enable_if_t<std::is_integral_v<T>, int> = 0> S(T);
  template <class T, std::enable_if_t<!std::is_integral_v<T>, int> = 0> S(T);
  template <class T, int N = 3, class U = long> int f(T) const;
  int v;
};
template <class C>
template <class T, std::enable_if_t<std::is_integral_v<T>, int>>
S<C>::S(T) : v(1) {}
template <class C>
template <class T, std::enable_if_t<!std::is_integral_v<T>, int>>
S<C>::S(T) : v(2) {}
template <class C>
template <class T, int N, class U>
int S<C>::f(T) const {
  return N + int(sizeof(U) > 4);
}

int main() {
  Cord c(std::string("ab"));
  S<char> a(1), b(1.0);
  std::printf("%d %d %d %d %d\n", c.n, a.v, b.v, a.f(0) + 1, a.f<int, 4, char>(0));
  return c.n - 2;
}
