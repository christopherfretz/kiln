// [temp.arg.nontype]/2, [temp.inst]: a non-type template parameter of type
// `_BitInt(N)` or `unsigned __int128` is replaced in the instantiation by a
// literal of the argument's value, also one above the `__int128` maximum.
// clang is the oracle.
template <_BitInt(7) V> constexpr _BitInt(7) f() { return V; }
static_assert(f<-3>() == -3);
template <unsigned _BitInt(7) V> constexpr auto f2() { return V; }
static_assert(f2<127>() == 127);
using u128 = unsigned __int128;
template <u128 V> constexpr u128 g() { return V; }
static_assert(g<~u128(0)>() == ~u128(0));
static_assert(g<u128(1) << 127>() == u128(1) << 127);
template <unsigned _BitInt(128) V> constexpr auto k() { return V; }
static_assert(k<~(unsigned _BitInt(128))0>() == ~(unsigned _BitInt(128))0);
