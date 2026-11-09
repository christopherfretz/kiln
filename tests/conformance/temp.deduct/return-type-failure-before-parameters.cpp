// [temp.deduct.general]/7: a leading return type is substituted before the
// parameter types, so its failure stops substitution before a parameter
// type whose instantiation would be a hard error. Covers a member template,
// a pointer-to-failed-type return and a non-type template parameter (WP-958).
//@ std: c++20
//@ expect: compile-ok
template <typename T> struct trap {
  typedef typename T::trap type;
};
template <bool, typename T = void> struct enable_if {
  typedef T type;
};
template <typename T> struct enable_if<false, T> {};

struct S {
  template <typename T>
  static typename enable_if<sizeof(T) == 17, int>::type *g(T, typename trap<T>::type = 0);
  static long g(...);
};
static_assert(sizeof(S::g(1)) == sizeof(long));

template <int N>
typename enable_if<N == 3>::type h(int (&)[N], typename trap<int[N]>::type * = 0);
int h(...);
int a[2];
static_assert(sizeof(h(a)) == sizeof(int));

template <typename T> struct Outer {
  template <typename U>
  typename enable_if<sizeof(U) == sizeof(T) + 99>::type k(U, typename trap<U>::type = 0);
  char k(...);
};
static_assert(sizeof(Outer<int>().k(1.0)) == 1);
