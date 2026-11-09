// The valid twin of pack-not-more-specialized.cpp and
// pack-does-not-specialize.cpp: partial specializations with packs that are
// more specialized than the primary, and a pack expansion as the argument
// list of a template with a defaulted parameter (no defaults are added after
// the expansion; clang PackExpansionIntoNonPack), and the deducible twins of
// pack-not-deducible.cpp. WP-956.
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
template <class... T> struct P { static constexpr int k = 0; };
template <class T, class... U> struct P<T, U...> { static constexpr int k = 1; };
template <class T, class... U> struct P<T *, U...> { static constexpr int k = 2; };
template <int... N> struct Q { static constexpr int k = 0; };
template <int... N> struct Q<0, N...> { static constexpr int k = 1; };
template <class T, class U = int, class V = U> struct X { static constexpr int k = 0; };
template <class... Ts> struct Y { using type = X<Ts...>; };
template <class T, class... Ts> constexpr int b = 0;
template <class... Ts> constexpr int b<int, Ts...> = 1;
template <class... T> struct L {};
template <class T> struct Z { static constexpr int k = 0; };
template <class... T, class V> struct Z<void(T..., V, T...)> {};
template <class... T, class V> struct Z<L<V, T...> *> { static constexpr int k = 1; };
template <int... N> struct W { static constexpr int k = 0; };
template <int... N> struct W<0, N...> { static constexpr int k = 1; };
static_assert(P<>::k == 0);
static_assert(P<int, char>::k == 1);
static_assert(P<int *>::k == 2);
static_assert(Q<1, 2>::k == 0);
static_assert(Q<0, 2>::k == 1);
static_assert(Y<char, long, short>::type::k == 0);
static_assert(Y<char>::type::k == 0);
static_assert(b<int, char> == 1 && b<char> == 0);
static_assert(Z<L<int, char> *>::k == 1 && Z<L<> *>::k == 0);
static_assert(W<0, 1>::k == 1 && W<1, 0>::k == 0);
int main() {}
