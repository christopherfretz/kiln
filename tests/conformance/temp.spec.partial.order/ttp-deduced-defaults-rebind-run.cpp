// [temp.spec.partial.order], [temp.deduct.partial]/12, [temp.arg.template]/3
// (P0522; CWG2398 as clang deduces it): ordering `R<Cont<V, A>, U>` of
// `template <class, class> class Cont` against `R<Cont<V>, U>` of
// `template <class> class Cont`, `Cont<V>` deduces from the synthesized
// `Cont'<V', A'>` with `A'` as `Cont`'s default, so substituting gives
// `Cont'<V', A'>` back and the two-parameter one is more specialized; the
// reverse fails (`A` has no argument). With the variadic
// `R<Cont<V, A, An...>, U>` of `template <class, class, class...> class Cont`
// as well (the container-rebind trio of Boost.Container, written for
// compilers without P0522), `vec<int>` is accepted: clang 19 and GCC pick
// the two-parameter one, clang 22 the variadic one, and every choice names
// `vec<long, int>` (WP-1212). The same holds nested in a pointer, reference
// or template argument; a one-parameter template takes the `Cont<V>` one, a
// template without defaults only the variadic one, and the C++03 chain the
// longest list.
//@ std: c++17
//@ expect: run
//@ stdout: trio 1 alloc 1 pair 22 nest 246 one 1 three 1 chain 3
extern "C" int printf(const char *, ...);
template <class X, class Y> struct same { static constexpr int v = 0; };
template <class X> struct same<X, X> { static constexpr int v = 1; };

template <class T, class A = int> struct vec {};
template <class T> struct one {};
template <class T, class A, class X> struct three {};

template <class C, class U> struct rebind;
template <template <class, class, class...> class Cont, class V, class A, class... An, class U>
struct rebind<Cont<V, A, An...>, U> { using type = Cont<U, A, An...>; };
template <template <class, class> class Cont, class V, class A, class U>
struct rebind<Cont<V, A>, U> { using type = Cont<U, A>; };
template <template <class> class Cont, class V, class U>
struct rebind<Cont<V>, U> { using type = Cont<U>; };

// The trio as the library writes it, with the allocator rebound.
template <class T> struct alloc {};
template <class V, class A, class U> struct rebind_alloc { using type = alloc<U>; };
template <class V, class U> struct rebind_alloc<V, void, U> { using type = void; };
template <class T, class A = alloc<T>> struct cvec {};
template <class Cont, class U> struct container_rebind;
template <template <class, class, class...> class Cont, typename V, typename A, class... An, class U>
struct container_rebind<Cont<V, A, An...>, U> {
  typedef Cont<U, typename rebind_alloc<V, A, U>::type, An...> type;
};
template <template <class, class> class Cont, typename V, typename A, class U>
struct container_rebind<Cont<V, A>, U> {
  typedef Cont<U, typename rebind_alloc<V, A, U>::type> type;
};
template <template <class> class Cont, typename V, class U>
struct container_rebind<Cont<V>, U> { typedef Cont<U> type; };

// The pair alone, in both declaration orders: the two-parameter one wins.
template <class C> struct pair_ab { static constexpr int v = 0; };
template <template <class, class> class Cont, class V, class A>
struct pair_ab<Cont<V, A>> { static constexpr int v = 2; };
template <template <class> class Cont, class V>
struct pair_ab<Cont<V>> { static constexpr int v = 1; };
template <class C> struct pair_ba { static constexpr int v = 0; };
template <template <class> class Cont, class V>
struct pair_ba<Cont<V>> { static constexpr int v = 1; };
template <template <class, class> class Cont, class V, class A>
struct pair_ba<Cont<V, A>> { static constexpr int v = 2; };

// Nested in a compound type, the same.
template <class X> struct nest { static constexpr int v = 0; };
template <template <class, class> class C, class V, class A>
struct nest<C<V, A> *> { static constexpr int v = 2; };
template <template <class> class C, class V> struct nest<C<V> *> { static constexpr int v = 1; };
template <template <class, class> class C, class V, class A>
struct nest<const C<V, A> &> { static constexpr int v = 4; };
template <template <class> class C, class V> struct nest<const C<V> &> { static constexpr int v = 3; };
template <template <class, class> class C, class V, class A>
struct nest<one<C<V, A>>> { static constexpr int v = 6; };
template <template <class> class C, class V> struct nest<one<C<V>>> { static constexpr int v = 5; };

// The C++03 chain: the longest matching list is the most specialized.
template <class T, class A = int, class O = void> struct vec3 {};
template <class C> struct chain { static constexpr int v = 0; };
template <template <class> class Cont, class V> struct chain<Cont<V>> { static constexpr int v = 1; };
template <template <class, class> class Cont, class V, class A>
struct chain<Cont<V, A>> { static constexpr int v = 2; };
template <template <class, class, class> class Cont, class V, class A, class P0>
struct chain<Cont<V, A, P0>> { static constexpr int v = 3; };

int main() {
  printf("trio %d ", same<rebind<vec<int>, long>::type, vec<long, int>>::v);
  printf("alloc %d ",
         same<container_rebind<cvec<int>, long>::type, cvec<long, alloc<long>>>::v);
  printf("pair %d%d ", pair_ab<vec<int>>::v, pair_ba<vec<int>>::v);
  printf("nest %d%d%d ", nest<vec<int> *>::v, nest<const vec<int> &>::v, nest<one<vec<int>>>::v);
  printf("one %d ", same<rebind<one<int>, long>::type, one<long>>::v);
  printf("three %d ", same<rebind<three<int, int, char>, long>::type, three<long, int, char>>::v);
  printf("chain %d\n", chain<vec3<int>>::v);
}
