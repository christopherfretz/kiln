//@ expect: run
//@ strict-diags
//@ stdout: 8 2 8 1 4 8
// [temp.expl.spec]/1, [temp.spec.partial], [temp.names]/3: a
// member template that only an explicit or partial specialization
// declares is named through the specialization's template-id
// (`O<int>::c<long>`); the primary template's members do not decide it.
#include <stdio.h>

template <class A, class B> struct same { static constexpr bool value = false; };
template <class A> struct same<A, A> { static constexpr bool value = true; };

template <class T> struct O { template <class I> using c = I *; };
template <> struct O<int> {
  template <class I> using c = I;
  template <class I> struct m { I v; };
  template <class I> static constexpr int vt = sizeof(I);
  template <class I> static constexpr int f() { return sizeof(I); }
};
template <class T> struct O<T *> { template <class I> struct p { T t; I i; }; };

O<int>::c<long> x = 8;
O<int>::m<short> y{2};
O<char>::c<long> z = &x;
O<char *>::p<int> w{1, 4};
static_assert(same<O<int>::c<long>, long>::value);
static_assert(same<O<char>::c<long>, long *>::value);
static_assert(O<int>::vt<long> == sizeof(long));
static_assert(O<int>::f<char>() == 1);

int main() {
  printf("%d %d %d %d %d %d\n", (int)x, (int)y.v, (int)*z, (int)w.t, w.i,
         O<int>::vt<double>);
}
