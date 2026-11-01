//@ expect: run
// [temp.over.link]/6-7: member templates (constructors, conversion
// functions, ordinary members) whose template heads differ only in the
// type of a defaulted non-type parameter, `enable_if_t<C1, int> = 0` vs
// `enable_if_t<C2, int> = 0`, are distinct overloads, in class templates
// and in plain classes; their out-of-line definitions match the right
// declaration, and SFINAE selects between them (libc++ `<optional>`).
template <bool B, class T = void> struct EI {};
template <class T> struct EI<true, T> { using type = T; };
template <bool B, class T = void> using EI_t = typename EI<B, T>::type;
template <class U> struct CK {
  template <class V> static constexpr bool imp() { return sizeof(V) == 4; }
  template <class V> static constexpr bool exp() { return sizeof(V) != 4; }
};
template <class A> struct S {
  int v;
  template <class X, EI_t<sizeof(X) == 4, int> = 0> S(X);
  template <class X, EI_t<sizeof(X) != 4, int> = 0> explicit S(X);
  template <class X, EI_t<sizeof(X) == 4, int> = 0> operator X*() { return nullptr; }
  template <class X, EI_t<sizeof(X) != 4, int> = 0> operator X*() { return nullptr; }
};
template <class A> template <class Y, EI_t<sizeof(Y) == 4, int>> S<A>::S(Y) : v(1) {}
template <class A> template <class Y, EI_t<sizeof(Y) != 4, int>> S<A>::S(Y) : v(2) {}
template <class A> struct T {
  int v;
  template <class X, EI_t<CK<X>::template imp<X>(), int> = 0> T(X) : v(3) {}
  template <class X, EI_t<CK<X>::template exp<X>(), int> = 0> explicit T(X) : v(4) {}
};
struct N {
  int v;
  template <class X, typename EI<sizeof(X) == 4, int>::type = 0> N(X);
  template <class X, typename EI<sizeof(X) != 4, int>::type = 0> explicit N(X);
};
template <class Y, typename EI<sizeof(Y) == 4, int>::type> N::N(Y) : v(10) {}
template <class Y, typename EI<sizeof(Y) != 4, int>::type> N::N(Y) : v(20) {}
int main() {
  S<int> a(1);
  S<int> b(2.0);
  T<int> c(1);
  T<int> d(2.0);
  N e(1);
  N f(2.0);
  int *p = a;
  double *q = a;
  return a.v == 1 && b.v == 2 && c.v == 3 && d.v == 4 && e.v == 10 && f.v == 20 && !p && !q
             ? 0
             : 1;
}
