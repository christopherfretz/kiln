// [temp.friend]/5, [temp.over.link]/6-7: a friend member function template
// of each A<T> declared in a member class template O<X>::B<Y> is matched
// in every specialization of B against the member templates of A<T> with
// the outer levels of the friend substituted (WP-1145): `template <class T>
// template <Y N> friend int A<T>::h(const B &)` names A<int>::h<int N> in
// B<int> but not in B<long> (its N has type long there), a type-constraint
// names the member with the equivalent one (g) and not one naming another
// concept (e). Denied access is observed through a requires-expression.
// clang does not model these friends (-Wunsupported-friend; clang 19
// warns only for the first one); GCC 16.1 rejects each of them ("no
// declaration matches").
//@ skip(clang): clang does not model [temp.friend]/5 friends (-Wunsupported-friend)
//@ expect: run
//@ strict-diags
//@ stdout: 7 0 7 0
//@ diag: 24:56 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 25:56 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 26:56 warning "for friend class declaration is not supported; turning off access control for 'B'"
template <class T> concept C = sizeof(T) > 1;
template <class T> concept E = sizeof(T) > 2;
template <class T> struct A;
template <class X> struct O {
  template <class Y> class B {
    template <class T> template <Y N> friend int A<T>::h(const B &);
    template <class T> template <C U> friend int A<T>::g(U, const B &);
    template <class T> template <C U> friend int A<T>::e(U, const B &);
    int n = 7;
  public:
    B() {}
  };
};
template <class T> struct A {
  template <int N> static int h(const typename O<T>::template B<int> &);
  template <int N> static int h(const typename O<T>::template B<long> &);
  template <C U> static int g(U, const typename O<T>::template B<int> &);
  template <E U> static int e(U, const typename O<T>::template B<int> &);
};
template <class T> template <int N> int A<T>::h(const typename O<T>::template B<int> &b) {
  return b.n;
}
template <class T> template <int N> int A<T>::h(const typename O<T>::template B<long> &b) {
  if constexpr (requires { b.n; }) return b.n;
  else return 0;
}
template <class T> template <C U> int A<T>::g(U, const typename O<T>::template B<int> &b) {
  return b.n;
}
template <class T> template <E U> int A<T>::e(U, const typename O<T>::template B<int> &b) {
  if constexpr (requires { b.n; }) return b.n;
  else return 0;
}
extern "C" int printf(const char *, ...);
int main() {
  O<int>::B<int> b;
  O<int>::B<long> c;
  printf("%d %d %d %d\n", A<int>::h<1>(b), A<int>::h<1>(c), A<int>::g(1, b), A<int>::e(1, b));
}
