// [temp.constr.normal]/1.4, /1.5, [temp.constr.fold]/4 (P2963R3): the
// pattern of a fold expanded constraint may be any constraint over the
// pack: a concept-id applied to the element (`(B<Ts> && ...)`), with
// further arguments (`(B2<Ts, int> && ...)`), or to a type built from it
// (`(B<Box<Ts>> && ...)`). Through a concept the fold expands what the
// concept-id gives its pack parameter: `AllB<Box<Ts>...>` is the fold over
// `Ts` of `B<Box<Ts>>`, `AllB<int, Ts...>` is `B<int> && (B<Ts> && ...)`,
// and a non-dependent `AllB<int, long>` is `B<int> && B<long>`. In each
// pair the overload whose fold's pattern subsumes the other's is more
// constrained (spec/sema-templates.md §8.2). WP-776.
//@ expect: compile-ok
//@ requires: clang>=22
template <class T> struct Box { using type = T; };
template <class T> concept A = sizeof(T) > 0;
template <class T> concept B = A<T> && (alignof(T) > 0);
template <class T, class U> concept A2 = sizeof(T) >= sizeof(U) || true;
template <class T, class U> concept B2 = A2<T, U> && (alignof(T) > 0);
template <class... Us> concept AllA = (A<Us> && ...);
template <class... Us> concept AllB = (B<Us> && ...);
template <class... Us> concept AnyA = (A<Us> || ...);
template <class... Us> concept AnyB = (B<Us> || ...);
template <class... Us> concept AllA2 = (A2<Us, int> && ...);
template <class... Us> concept AllB2 = (B2<Us, int> && ...);
template <int N> concept NA = N >= 0 || true;
template <int N> concept NB = NA<N> && (N != -12345);
template <int... Ms> concept AllNA = (NA<Ms> && ...);
template <int... Ms> concept AllNB = (NB<Ms> && ...);
template <class... Vs> concept ViaA = AllA<Vs*...>;
template <class... Vs> concept ViaB = AllB<Vs*...>;

template <class... Ts> requires (A<Ts> && ...) constexpr int f1(Ts...) { return 1; }
template <class... Ts> requires (B<Ts> && ...) constexpr int f1(Ts...) { return 2; }
static_assert(f1(1, 2) == 2);
template <class... Ts> requires (A2<Ts, int> && ...) constexpr int f2(Ts...) { return 1; }
template <class... Ts> requires (B2<Ts, int> && ...) constexpr int f2(Ts...) { return 2; }
static_assert(f2(1, 2) == 2);
template <class... Ts> requires (A<Box<Ts>> && ...) constexpr int f3(Ts...) { return 1; }
template <class... Ts> requires (B<Box<Ts>> && ...) constexpr int f3(Ts...) { return 2; }
static_assert(f3(1, 2) == 2);
template <class... Ts> requires AllA<Box<Ts>...> constexpr int f4(Ts...) { return 1; }
template <class... Ts> requires AllB<Box<Ts>...> constexpr int f4(Ts...) { return 2; }
static_assert(f4(1, 2) == 2);
template <class... Ts> requires AnyA<Box<Ts>...> constexpr int f5(Ts...) { return 1; }
template <class... Ts> requires AnyB<Box<Ts>...> constexpr int f5(Ts...) { return 2; }
static_assert(f5(1, 2) == 2);
template <class... Ts> requires AllA2<Ts*...> constexpr int f6(Ts...) { return 1; }
template <class... Ts> requires AllB2<Ts*...> constexpr int f6(Ts...) { return 2; }
static_assert(f6(1, 2) == 2);
// A plain element: the fold written in the template and the one through
// the concept are the same constraint.
template <class... Ts> requires (A<Ts> && ...) constexpr int f7(Ts...) { return 1; }
template <class... Ts> requires AllB<Ts...> constexpr int f7(Ts...) { return 2; }
static_assert(f7(1, 2) == 2);
template <class... Ts> requires AllA<Ts...> constexpr int f8(Ts...) { return 1; }
template <class... Ts> requires (B<Ts> && ...) constexpr int f8(Ts...) { return 2; }
static_assert(f8(1, 2) == 2);
// Mixed pack arguments.
template <class... Ts> requires AllA<int, Ts...> constexpr int f9(Ts...) { return 1; }
template <class... Ts> requires AllB<int, Ts...> constexpr int f9(Ts...) { return 2; }
static_assert(f9(1, 2) == 2);
template <class... Ts> requires AllA<Ts..., int> constexpr int f10(Ts...) { return 1; }
template <class... Ts> requires AllB<Ts..., int> constexpr int f10(Ts...) { return 2; }
static_assert(f10(1, 2) == 2);
template <class T, class... Ts> requires AllA<T, Ts...> constexpr int f11(T, Ts...) { return 1; }
template <class T, class... Ts> requires AllB<T, Ts...> constexpr int f11(T, Ts...) { return 2; }
static_assert(f11(1, 2) == 2);
template <class... Ts> requires AllA<int, Ts...> constexpr int f12(Ts...) { return 1; }
template <class... Ts> requires (A<int> && AllB<Ts...>) constexpr int f12(Ts...) { return 2; }
static_assert(f12(1, 2) == 2);
template <class... Ts> requires AllA<Ts..., Ts...> constexpr int f13(Ts...) { return 1; }
template <class... Ts> requires AllB<Ts...> constexpr int f13(Ts...) { return 2; }
static_assert(f13(1, 2) == 2);
template <class... Ts> requires AnyA<int, Box<Ts>...> constexpr int f14(Ts...) { return 1; }
template <class... Ts> requires AnyB<int, Box<Ts>...> constexpr int f14(Ts...) { return 2; }
static_assert(f14(1, 2) == 2);
template <class T> requires (AllA<int, long> && A<T>) constexpr int f15(T) { return 1; }
template <class T> requires (AllB<int, long> && A<T>) constexpr int f15(T) { return 2; }
static_assert(f15(1) == 2);
// Non-type packs, a member template, a chain of concepts.
template <int... Ns> requires AllNA<(Ns * 2)...> constexpr int g1() { return 1; }
template <int... Ns> requires AllNB<(Ns * 2)...> constexpr int g1() { return 2; }
static_assert(g1<1, 2>() == 2);
template <class T> struct Outer {
  template <class... Ts> requires AllA<Box<Ts>...> static constexpr int m(Ts...) { return 1; }
  template <class... Ts> requires AllB<Box<Ts>...> static constexpr int m(Ts...) { return 2; }
};
static_assert(Outer<int>::m(1, 2) == 2);
template <class... Ts> requires ViaA<Box<Ts>...> constexpr int g2(Ts...) { return 1; }
template <class... Ts> requires ViaB<Box<Ts>...> constexpr int g2(Ts...) { return 2; }
static_assert(g2(1, 2) == 2);
