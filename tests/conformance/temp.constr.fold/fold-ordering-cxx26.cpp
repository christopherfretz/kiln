// [temp.constr.fold] (P2963, C++26): fold expanded constraints over the same
// packs subsume element-wise, so the more constrained overloads win; the
// C++20 sibling fold-atomic-before-cxx26 is ambiguous.
//@ std: c++26
//@ expect: compile-ok
template <class T> concept Small = sizeof(T) <= 8;
template <class T> concept Tiny = Small<T> && sizeof(T) <= 4;
template <Small... Ts> constexpr int f(Ts...) { return 0; }
template <Tiny... Ts> constexpr int f(Ts...) { return 1; }
template <class... Ts> requires (Small<Ts> && ...) constexpr int g(Ts...) { return 0; }
template <class... Ts> requires (Tiny<Ts> && ...) constexpr int g(Ts...) { return 1; }
static_assert(f(1, 2) == 1, "");
static_assert(g(1) == 1, "");
static_assert(f(1.0) == 0, "");
template <class... Ts> concept AllSmall = (Small<Ts> && ...);
template <class... Ts> requires AllSmall<Ts...> constexpr int h(Ts...) { return 0; }
template <class... Ts> requires AllSmall<Ts...> && (sizeof...(Ts) > 1) constexpr int h(Ts...) { return 1; }
static_assert(h(1, 2) == 1, "");
static_assert(h(1) == 0, "");
