// [over.match.class.deduct]/1.5-1.8 with [temp.mem]: the aggregate
// deduction candidate of a member class template of a class template
// specialization (`A<int>::B`) takes the elements of the member template's
// definition with the enclosing arguments substituted, though that member
// template has no definition of its own yet; a trailing pack expansion base
// stays a pack. Braced and parenthesized forms, from outside and from a
// member function of the enclosing template. Found running GCC's C++
// testsuite.
//@ expect: run
//@ strict-diags
//@ std: c++20
//@ stdout: 2 3 7 x 5 1
#include <cstdio>
template <class T1> struct A {
  template <class T2> struct B { T2 t; };
  template <class T2, class U> struct P { T1 a; T2 b; U c; };
};
template <class> struct E {
  template <class T> struct G { T t; };
  int fn() { G g{5}; return g.t; }
};
template <class... Ts> struct Base : Ts... {};
struct X { int x = 1; };
template <class> struct Outer {
  template <class... Fn> struct V : Fn... {};
};
int main() {
  A<int>::B x{2};
  A<long>::B y(3);
  A<int>::P p{7, 'x', 2.5};
  Outer<int>::V v{X{}};
  std::printf("%d %d %d %c %d %d\n", x.t, y.t, p.a, p.b, E<int>().fn(), v.x);
}
