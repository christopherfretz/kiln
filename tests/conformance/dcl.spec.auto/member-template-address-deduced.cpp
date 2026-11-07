// [dcl.spec.auto.general]/12, [expr.unary.op]/3: `&C::f<T>` naming a
// member function template specialization (of a class or of a class
// template specialization) with a placeholder return type instantiates its
// definition to deduce that type; the pointer to member has the deduced
// type, also as an `auto` template argument.
//@ expect: run
//@ std: c++20
//@ stdout: 1 1 5 9
#include <cstdio>
#include <type_traits>
struct R { template<class T> auto emp(int e) { return e + 1; } };
template<class E> struct Q {
  template<class T, class... A> decltype(auto) emp(E e, A &&...a) { return T{e, a...}; }
};
struct C { int x; char y; };
template<auto Cand> int g() { using D = decltype(Cand); return std::is_same_v<D, int (R::*)(int)>; }
template<auto Cand> int h() {
  using D = decltype(Cand);
  return std::is_same_v<D, C (Q<int>::*)(int, const char &)>;
}
int main() {
  auto p = &R::emp<char>;
  Q<int> q;
  auto m = &Q<int>::emp<C, const char &>;
  char y = 4;
  std::printf("%d %d %d %d\n", g<&R::emp<char>>(), h<&Q<int>::emp<C, const char &>>(),
              (R{}.*p)(4), (q.*m)(5, y).x + y);
}
