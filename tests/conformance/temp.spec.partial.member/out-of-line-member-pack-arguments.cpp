//@ expect: run
//@ stdout: 12 22 3 4 215 3
// [temp.spec.partial.member]/1, [temp.dep.type]/1.2: an out-of-line member
// of a partial specialization whose arguments include a pack expansion for
// the primary's template parameter pack names that partial specialization
// (its own arguments hold the expansion as one argument pack; folly
// Format-inl.h BaseFormatterImpl).
#include <cstdio>
template <decltype(sizeof 0)... I> struct seq {};
template <class... T> struct L {};
template <class... A> struct B;
template <class... A> struct B<int, A...> {
  int g() const;
  template <class O> int h(O) const;
  struct N;
  static int s;
};
template <class... A> int B<int, A...>::g() const { return 10 + sizeof...(A); }
template <class... A> template <class O> int B<int, A...>::h(O o) const { return o + sizeof...(A); }
template <class... A> struct B<int, A...>::N { int v = 3; };
template <class... A> int B<int, A...>::s = 4;
template <class D, bool c, class Ind, class... A> struct F;
template <class D, bool c, decltype(sizeof 0)... I, class... A>
struct F<D, c, seq<I...>, A...> {
  template <class O> int f(O& o) const;
};
template <class D, bool c, decltype(sizeof 0)... I, class... A>
template <class O>
int F<D, c, seq<I...>, A...>::f(O& o) const { return sizeof...(I) * 100 + sizeof...(A) * 10 + o; }
template <class T, class... U> struct P;
template <class T, class... U> struct P<L<T>, U...> { int g(); };
template <class T, class... U> int P<L<T>, U...>::g() { return sizeof...(U); }
int main() {
  B<int, char, long> b;
  F<int, true, seq<1, 2>, char> f;
  int o = 5;
  std::printf("%d %d %d %d %d %d\n", b.g(), b.h(20), B<int, char>::N().v, B<int>::s, f.f(o),
              P<L<int>, int, int, int>().g());
}
