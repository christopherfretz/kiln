//@ expect: compile-fail
// [temp.variadic]/8: packs expanded together must have the same length.
template <class... Ts> struct tl {};
template <class... A> struct zip { template <class... B> using with = tl<tl<A, B>...>; };
template <class... A, class... B> constexpr int pairs(tl<A...>, tl<B...>) { return (0 + ... + (sizeof(A) + sizeof(B))); }
int x = pairs(tl<int>{}, tl<int, int>{});
