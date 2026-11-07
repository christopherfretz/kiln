// [temp.friend]/9: "Such a constrained friend function or function
// template declaration does not declare the same function or function
// template as a declaration in any other scope." A non-template friend
// with a requires-clause defined in A<char> and A<int> is two functions
// (no redefinition); so is a friend template whose type-constraint names
// the enclosing class template's parameter, and one whose constraint names
// the parameter of an enclosing class template two levels out.
//@ expect: run
//@ stdout: 1 4 2 3 7 7
#include <concepts>
#include <cstdio>

struct Base {};
template <class T> struct A : Base {
  friend int f(Base) requires(sizeof(T) > 0) { return sizeof(T); }
};
template <class T> struct B : Base {
  template <std::convertible_to<T> U, class V = T>
  friend V h(Base, U u) { return static_cast<V>(u) + 1; }
};
template <class T> struct O {
  template <class X> struct I : Base {
    template <class U, class W = T>
      requires(sizeof(U) == sizeof(T))
    friend W k(Base, U) { return 7; }
  };
};

int main() {
  A<char> a1;
  A<int> a2;
  B<int> b1;
  B<long> b2;
  O<int>::I<char> i1;
  O<long>::I<char> i2;
  std::printf("%d %d %d %ld %d %ld\n", f(a1), f(a2), h(b1, 1), h(b2, 2), k(i1, 1), k(i2, 1L));
}
