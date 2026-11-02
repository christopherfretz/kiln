// [temp.over.link]/5-6: a non-dependent non-type argument in a dependent
// specialization (`It<C, false>`) is equivalent across declarations, so
// the definition redeclares the earlier function template (libc++
// __fill_n_bool in fill_n.h + __bit_reference; bitset tests).
//@ expect: run
//@ stdout: 1 2 3
#include <cstdio>
template <class C, bool B> class It;
template <class C> int fill(It<C, false> f);
template <class C, int N> struct A {};
template <class C> int get(A<C, 2>);
template <class C> int get(A<C, 1 + 2>);

template <class C, bool B> class It {
  template <class D> friend int fill(It<D, false> f);
  int priv = 1;
};
template <class C> int fill(It<C, false> f) { return f.priv; }
template <class C> int get(A<C, 2>) { return 2; }
template <class C> int get(A<C, 1 + 2>) { return 3; }

struct Cont {};
int main() {
  std::printf("%d %d %d\n", fill(It<Cont, false>{}), get(A<int, 2>{}), get(A<int, 3>{}));
}
