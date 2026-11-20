// [temp.friend]/5: a friend declaration with its own template head whose
// nested-name-specifier names a specialization of a class template, the
// head declaring a non-type or a template template parameter, in a class
// template: each specialization of `B` befriends the member of every
// specialization the head deduces from (a member function, a member
// function of `T2<TT>`, a member class template, and a nested class of
// the class template itself). Instantiating the friend redeclares the
// head's parameters one level shallower (spec/sema-templates.md §4.6).
// Reduced from g++.dg template/friend55.C, friend61.C and friend85.C.
//@ expect: run
//@ oracle: gcc
//@ skip(clang): clang does not model [temp.friend]/5 friends (-Wunsupported-friend)
//@ strict-diags
//@ diag: 28:37 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 29:59 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 30:59 warning "dependent nested name specifier"
//@ diag: 42:45 warning "for friend class declaration is not supported; turning off access control for 'Trick'"
//@ stdout: 44 100 1004 3
#include <cstdio>
template <int I> struct A {
  int impl();
  template <class U> struct Inner;
};
template <template <class> class TT> struct T2 { int get(); };
template <class> struct Box {};
template <class X> class B {
  int i = sizeof(X);
  template <int N> friend int A<N>::impl();
  template <template <class> class TT> friend int T2<TT>::get();
  template <int N> template <class U> friend struct A<N>::Inner;
};
B<int> b1;
B<char> b2;
template <int I> template <class U> struct A<I>::Inner {
  static int peek() { return b1.i + U::v; }
};
template <template <class> class TT> int T2<TT>::get() { return b2.i * 100; }
template <int I> int A<I>::impl() { return b1.i * 10 + b2.i + I; }
struct V { static constexpr int v = 1000; };
template <int V> struct Outer {
  class Trick {
    template <int W> friend class Outer<W>::Trick;
    int secret = V;
  public:
    template <int W> int peek(const typename Outer<W>::Trick &t) const {
      return t.secret + secret;
    }
  };
};
int main() {
  int a = A<3>().impl();
  int t = T2<Box>().get();
  int p = A<1>::Inner<V>::peek();
  int s = Outer<1>::Trick().peek<2>(Outer<2>::Trick());
  std::printf("%d %d %d %d\n", a, t, p, s);
}
