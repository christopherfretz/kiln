// [temp.friend]/9, [temp.constr.decl]/3 (WP-231): a friend function
// template whose constraints name the enclosing class template's
// parameter is checked with that specialization's arguments outside its
// own (template-head and trailing requires-clauses, mixed with its own
// parameters).
//@ expect: run
//@ stdout: 1 2 3 4
#include <cstdio>
template <class T> concept C = sizeof(T) == 4;
template <class T> struct X {
  template <class U> friend int f(X, U) requires C<T> { return 1; }
  template <class U> requires C<T> friend int g(X, U) { return 2; }
  template <class U> friend int h(X, U) requires C<U> && C<T> { return 3; }
};
template <class T> struct Y {
  template <class U> friend int k(Y, U) requires (sizeof(T) == 8) { return 4; }
};
int main() {
  std::printf("%d %d %d %d\n", f(X<int>{}, 'c'), g(X<float>{}, 1L), h(X<int>{}, 1.f),
              k(Y<long>{}, 1));
}
