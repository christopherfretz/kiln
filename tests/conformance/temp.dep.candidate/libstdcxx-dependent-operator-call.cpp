// [temp.dep.general]/2, [temp.dep.candidate], [basic.lookup.argdep]: an
// unqualified call written with an operator-function-id, `operator<=>(t, u)`,
// whose arguments are type-dependent is a dependent call; unqualified lookup
// finding nothing at definition time is not an error, and ADL at
// instantiation finds the operator. libstdc++ <compare>:535 (the
// `__3way_builtin_ptr_cmp` requires-expression), reached from every header
// that includes <compare> (docs/status/libstdcxx.md, WP-188). Verified
// against clang 19.
//@ expect: compile-ok
namespace N {
struct A { int v; };
constexpr int operator<=>(A x, A y) { return x.v - y.v; }
constexpr bool operator==(A x, A y) { return x.v == y.v; }
}
template<typename T, typename U>
concept has_free_3way = requires(T t, U u) { operator<=>(t, u); };
template<typename T>
constexpr int cmp(T a, T b) { return operator<=>(a, b); }
template<typename T>
constexpr bool eq(T a, T b) { return operator==(a, b); }
static_assert(has_free_3way<N::A, N::A>);
static_assert(!has_free_3way<int, int>);
static_assert(cmp(N::A{1}, N::A{2}) < 0);
static_assert(eq(N::A{1}, N::A{1}));
