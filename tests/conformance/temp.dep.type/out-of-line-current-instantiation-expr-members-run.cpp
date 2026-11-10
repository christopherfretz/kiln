// [temp.dep.type]/6, [temp.over.link]/5, [temp.mem]/1: the expressions
// before the declarator-id are rebuilt in the current instantiation also
// for a static data member, a member of a member class and a member
// template whose expression names its own parameter
// (`I<C<X>::N + M> C<X>::k()`).
//@ expect: run
//@ strict-diags
//@ stdout: 9 12 3 6 2 5
extern "C" int printf(const char*, ...);
template <int N> struct I { static constexpr int v = N; };
template <class X> struct C {
  using T = X;
  static constexpr int N = sizeof(X);
  static decltype(T()) sv;
  static I<N * 3> si;
  struct In {
    decltype(T()) f();
    I<N + 2> g();
  };
  template <class Y> decltype(T() + Y()) h(Y);
  template <int M> I<N + M> k();
};
template <class X> decltype(typename C<X>::T()) C<X>::sv = 9;
template <class X> I<C<X>::N * 3> C<X>::si = {};
template <class X> decltype(typename C<X>::T()) C<X>::In::f() { return 3; }
template <class X> I<C<X>::N + 2> C<X>::In::g() { return {}; }
template <class X> template <class Y> decltype(typename C<X>::T() + Y()) C<X>::h(Y y) { return y; }
template <class X> template <int M> I<C<X>::N + M> C<X>::k() { return {}; }
int main() {
  C<int>::In in;
  C<int> c;
  printf("%d %d %d %d %d %d\n", C<int>::sv, decltype(C<int>::si)::v, in.f(), decltype(in.g())::v,
         (int)c.h(2.5), decltype(c.k<1>())::v);
}
