// [temp.constr.atomic], [temp.dep.candidate], [temp.point]: an unqualified
// call in a concept's constraint is looked up from the concept's
// definition, whatever namespace checks satisfaction. libc++'s
// `ranges::__swap::__unqualified_swappable_with` must find the deleted
// poison-pill `swap`, not `std::swap`, when `std::is_constructible_v`
// checks it from namespace `std`.
//@ std: c++26
namespace N {
template <class T> void swap(T &, T &) noexcept {}
namespace S {
template <class T> void swap(T &, T &) = delete;
template <class T>
concept uswap = requires(T &a) { swap(a, a); };
template <class T>
concept exch = !uswap<T>;
} // namespace S
template <class T> inline constexpr bool in_n = S::uswap<T>;
template <class T> inline constexpr bool exch_n = S::exch<T>;
template <class T>
  requires S::exch<T>
constexpr int pick(T &) { return 1; }
template <class T> constexpr int pick(T &) { return 2; }
} // namespace N

struct I {};
struct J {};
struct K {};
static_assert(!N::in_n<I>);
static_assert(N::exch_n<J>);
static_assert(!N::S::uswap<I>);
K k;
static_assert(N::pick(k) == 1);

// A type whose own namespace has a viable `swap`: found by ADL.
namespace M {
struct A {};
void swap(A &, A &) {}
} // namespace M
static_assert(N::in_n<M::A>);
static_assert(!N::exch_n<M::A>);
