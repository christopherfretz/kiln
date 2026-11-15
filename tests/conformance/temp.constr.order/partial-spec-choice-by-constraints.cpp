// [temp.spec.partial.match]/1, [temp.spec.partial.order], [temp.constr.order]/5:
// which partial specialization is chosen when the candidates differ only by
// constraints: the more constrained one, for a class template, a member class
// template of a class template instantiated through the enclosing
// specialization, and a variable template; a constrained explicit
// specialization's arguments win over everything. Each choice is a
// static_assert, so a wrong choice cannot pass silently (written beside the
// fixes for wrong-code found running GCC's C++ testsuite).
//@ expect: compile-ok
//@ strict-diags
//@ std: c++20
template <class T>
concept Small = sizeof(T) <= 4;
template <class T>
concept SmallInt = Small<T> && __is_same(T, int);

template <class T>
struct A {
  static constexpr int v = 0;
};
template <Small T>
struct A<T> {
  static constexpr int v = 1;
};
template <SmallInt T>
struct A<T> {
  static constexpr int v = 2;
};
template <>
struct A<char> {
  static constexpr int v = 3;
};
static_assert(A<long>::v == 0);
static_assert(A<short>::v == 1);
static_assert(A<int>::v == 2);
static_assert(A<char>::v == 3);

template <class O>
struct D {
  template <class U>
  struct M {
    static constexpr int v = 0;
  };
  template <Small U>
  struct M<U *> {
    static constexpr int v = 1;
  };
  template <SmallInt U>
  struct M<U *> {
    static constexpr int v = 2;
  };
  template <SmallInt U>
    requires Small<O>
  struct M<U &> {
    static constexpr int v = 3;
  };
};
static_assert(D<int>::M<long *>::v == 0);
static_assert(D<int>::M<short *>::v == 1);
static_assert(D<int>::M<int *>::v == 2);
static_assert(D<int>::M<int &>::v == 3);
static_assert(D<long>::M<int &>::v == 0);

template <class T>
constexpr int vv = 0;
template <Small T>
constexpr int vv<T> = 1;
template <SmallInt T>
constexpr int vv<T> = 2;
static_assert(vv<long> == 0 && vv<short> == 1 && vv<int> == 2);

// A pointer non-type parameter and an enumeration one, more constrained by
// concepts on the values.
enum class K { a, b };
int g1, g2;
template <K k>
concept IsB = k == K::b;
template <K k, int *p>
concept IsBAtG2 = IsB<k> && p == &g2;
template <K k, int *p>
struct N {
  static constexpr int v = 0;
};
template <K k, int *p>
  requires IsB<k>
struct N<k, p> {
  static constexpr int v = 1;
};
template <K k, int *p>
  requires IsBAtG2<k, p>
struct N<k, p> {
  static constexpr int v = 2;
};
static_assert(N<K::a, &g2>::v == 0);
static_assert(N<K::b, &g1>::v == 1);
static_assert(N<K::b, &g2>::v == 2);
