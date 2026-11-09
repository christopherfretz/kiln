// [temp.spec.partial.order], [temp.constr.order], [temp.spec.partial.member]:
// which partial specialization is chosen. Equally specialized by deduction
// with equivalent heads and arguments, the more constrained one wins,
// type-constraints and requires-clauses alike, for class and variable
// templates and for member templates of an instantiated class template
// (in-class and out-of-line); otherwise deduction decides (CWG1432 for a
// trailing pack). WP-1053.
//@ expect: run
//@ std: c++20
//@ stdout: 2 1 0 | 2 1 0 | 2 1 0 | 1 2 0 | 3 5 | 2 1 0
extern "C" int printf(const char *, ...);

template <class T> concept C = sizeof(T) > 1;
template <class T> concept D = C<T> && sizeof(T) > 2;

template <class T, class U> struct W { static constexpr int v = 0; };
template <class T, class U> requires C<T> struct W<T *, U *> { static constexpr int v = 1; };
template <class T, class U> requires D<T> struct W<T *, U *> { static constexpr int v = 2; };
static_assert(W<int *, char *>::v == 2 && W<short *, char *>::v == 1 && W<char *, int *>::v == 0);

template <class T> constexpr int vt = 0;
template <C T> constexpr int vt<T *> = 1;
template <D T> constexpr int vt<T *> = 2;
static_assert(vt<int *> == 2 && vt<short *> == 1 && vt<char *> == 0);

template <class X> struct O {
  template <class T> struct A { static constexpr int v = 0; };
  template <class T> requires C<T> struct A<T *> { static constexpr int v = 1; };
  template <class T> requires D<T> struct A<T *> { static constexpr int v = 2; };
  template <class T, class... Vs> struct B { static constexpr int v = 0; };
  template <class T> struct B<T, X> { static constexpr int v = 1; };
  template <class T, class... Vs> struct B<T, X, Vs...> { static constexpr int v = 2; };
  template <class T> struct H { static constexpr int v = 0; };
};
template <class X> template <class T> requires C<T> struct O<X>::H<T *> { static constexpr int v = 1; };
template <class X> template <D T> struct O<X>::H<T *> { static constexpr int v = 2; };
static_assert(O<int>::A<int *>::v == 2 && O<int>::A<short *>::v == 1 && O<int>::A<char *>::v == 0);

template <class... Ts> struct S { static constexpr int v = 0; };
template <class T, class... Ts> struct S<T, Ts...> { static constexpr int v = 1; };
template <class T> struct S<T> { static constexpr int v = 3; };
template <class T, class... Ts> struct R { static constexpr int v = 0; };
template <class T, class... Ts> struct R<T *, Ts...> { static constexpr int v = 1; };
template <class T, class U> struct R<T *, U> { static constexpr int v = 5; };

int main() {
  printf("%d %d %d | ", W<long *, int *>::v, W<short *, int *>::v, W<char *, int *>::v);
  printf("%d %d %d | ", vt<long *>, vt<short *>, vt<char *>);
  printf("%d %d %d | ", O<int>::A<long *>::v, O<int>::A<short *>::v, O<int>::A<char *>::v);
  printf("%d %d %d | ", O<int>::B<int, int>::v, O<int>::B<int, int, char>::v, O<int>::B<int, char>::v);
  printf("%d %d | ", S<int>::v, R<int *, int>::v);
  printf("%d %d %d\n", O<long>::H<long *>::v, O<long>::H<short *>::v, O<long>::H<char *>::v);
}
static_assert(O<long>::H<int *>::v == 2 && O<long>::H<short *>::v == 1 && O<long>::H<char *>::v == 0);
