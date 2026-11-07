// [temp.deduct.partial]/12, [temp.func.order]/3: after deducing one
// template's parameters from the other's compared parameter types, a
// parameter left without a value fails the deduction when those types name
// it anywhere -- a non-deduced context, a nested-name-specifier, an
// expression (`sizeof(T)`, `N + 1`, `X<T>::value`) -- whatever its kind:
// type, non-type, template template parameter or pack (clang
// `MarkUsedTemplateParameters` without `OnlyDeduced`). Names that vanish
// from the canonical type (an alias template dropping its parameter, a
// non-dependent `decltype`) do not count, nor does the return type.
//@ expect: run
//@ stdout: 2 3 4 5 6 7 8 9 10 21 11
#include <cstddef>
#include <cstdio>

struct It {};
template <std::size_t N> struct SelN { using type = It; };
template <template <class> class TT> struct SelT { using type = It; };
template <class... Ts> struct SelP { using type = It; };
template <class T> struct V { static constexpr int value = 0; };
template <class T> struct Box {};
template <class T> struct Id { using type = T; };
template <class T> using Drop = It;

template <int N, class O> int fn(O, int) { return 1; }
template <int N> int fn(typename SelN<N>::type, int) { return 2; }
template <template <class> class TT, class O> int ft(O, int) { return 1; }
template <template <class> class TT> int ft(typename SelT<TT>::type, int) { return 3; }
template <class O, class... Ts> int fp(O, int) { return 1; }
template <class... Ts> int fp(typename SelP<Ts...>::type, int) { return 4; }
template <class T, class O> int fs(O) { return 1; }
template <class T> int fs(typename SelN<sizeof(T)>::type) { return 5; }
template <class T, class O> int fv(O) { return 1; }
template <class T> int fv(typename SelN<V<T>::value>::type) { return 6; }
template <int N, class O> int fe(O) { return 1; }
template <int N> int fe(typename SelN<N + 1>::type) { return 7; }
template <class T, class O> int fi(O) { return 1; }
template <class T> int fi(Box<typename Id<T>::type>) { return 8; }
// Not named once canonical: both directions deduce, the more specialized
// parameter (`It` against `O`) decides.
template <class T, class O> int fa(O, int) { return 1; }
template <class T> int fa(Drop<T>, int) { return 9; }
template <class T, class O> int fd(O, int) { return 1; }
template <class T> int fd(decltype(sizeof(T*)), int) { return 10; }
// Named only by the return type.
template <int N, class U> int g(U) { return 1; }
template <int N, class U> int g(U*) { return 2; }
template <template <class> class TT, class U> TT<U>* h(U) { return nullptr; }
template <template <class> class TT, class U> int h(U*) { return 11; }

int main() {
  It it;
  int i = 0;
  std::printf("%d %d %d %d %d %d %d %d %d %d %d\n", fn<0>(it, 1), ft<Box>(it, 1),
              fp<int, char>(it, 1), fs<int>(it), fv<int>(it), fe<0>(it), fi<int>(Box<int>{}),
              fa<int>(it, 0), fd<int>(sizeof(int*), 0), g<0>(i) + 10 * g<0>(&i), h<Box>(&i));
}
