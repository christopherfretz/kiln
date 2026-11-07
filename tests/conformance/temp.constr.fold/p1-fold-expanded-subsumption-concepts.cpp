// [temp.constr.fold], [temp.constr.normal]/1.4 (P2963R3): fold expanded
// constraints reached through concepts over packs (`AllA<Ts...>` with
// `concept AllA = (A<Ts> && ...)`) expand the pack the concept-id passes,
// so they subsume a fold written directly over that pack and each other;
// also non-type packs, partial specializations, and a pattern over two
// packs. clang 19 orders only the folds written directly (hence clang>=22)
// (spec/sema-templates.md §8.5). WP-771.
//@ expect: compile-ok
//@ requires: clang>=22
template <class T> concept A = sizeof(T) >= 1;
template <class T> concept B = __is_integral(T);
template <class T> concept C = A<T> && B<T>;
// [temp.constr.fold] example
template <class... T> requires (A<T> && ...) constexpr int f() { return 1; }
template <class... T> requires (C<T> && ...) constexpr int f() { return 2; }
static_assert(f<int>() == 2);
static_assert(f<double>() == 1);
// through concepts over packs
template <class... Ts> concept AllA = (A<Ts> && ...);
template <class... Ts> concept AllC = (C<Ts> && ...);
template <class... Ts> requires AllA<Ts...> constexpr int g(Ts...) { return 1; }
template <class... Ts> requires AllC<Ts...> constexpr int g(Ts...) { return 2; }
static_assert(g(1, 2) == 2);
static_assert(g(1.0) == 1);
// concept vs direct fold
template <class... Ts> requires AllA<Ts...> constexpr int h(Ts...) { return 1; }
template <class... Ts> requires (C<Ts> && ...) constexpr int h(Ts...) { return 2; }
static_assert(h(1) == 2);
// NTTP packs
template <int N> concept Pos = N > 0;
template <int N> concept Big = Pos<N> && (N > 10);
template <int... Ns> requires (Pos<Ns> && ...) constexpr int k() { return 1; }
template <int... Ns> requires (Big<Ns> && ...) constexpr int k() { return 2; }
static_assert(k<11, 12>() == 2);
static_assert(k<1>() == 1);
// partial specializations
template <class... Ts> struct S { static constexpr int v = 0; };
template <class... Ts> requires (A<Ts> && ...) struct S<Ts...> { static constexpr int v = 1; };
template <class... Ts> requires (C<Ts> && ...) struct S<Ts...> { static constexpr int v = 2; };
static_assert(S<int, long>::v == 2);
static_assert(S<int, double>::v == 1);
// concept with extra param and pack
template <class U, class... Ts> concept AllConv = (__is_convertible(Ts, U) && ...);
template <class U, class... Ts> concept AllConvC = AllConv<U, Ts...> && (C<Ts> && ...);
template <class... Ts> requires AllConv<long, Ts...> constexpr int m(Ts...) { return 1; }
template <class... Ts> requires AllConvC<long, Ts...> constexpr int m(Ts...) { return 2; }
static_assert(m(1, 2) == 2);
// fold in fold pattern? nested pack: pattern over two packs
template <class... Ts> struct L {};
template <class... Ts, class... Us> requires ((A<Ts> && A<Us>) && ...) constexpr int q(L<Ts...>, L<Us...>) { return 1; }
template <class... Ts, class... Us> requires ((C<Ts> && A<Us>) && ...) constexpr int q(L<Ts...>, L<Us...>) { return 2; }
static_assert(q(L<int>{}, L<int>{}) == 2);
