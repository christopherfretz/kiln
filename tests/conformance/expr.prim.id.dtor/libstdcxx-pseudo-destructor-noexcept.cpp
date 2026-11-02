// [expr.prim.id.dtor]/2, [expr.unary.noexcept]/2, [except.spec]: a
// pseudo-destructor call on a scalar is not potentially-throwing, so
// `{ t.~T() } noexcept` holds for int. libstdc++ concepts:130
// (__destructible_impl) makes destructible<char*> false, cascading through
// contiguous_iterator into <span>, <format>, ranges::subrange; 24 headers (docs/status/libstdcxx.md, WP-188).
// Verified against clang 19.
//@ expect: compile-ok
template<class T> concept A = requires(T &t) { t.~T(); };
template<class T> concept B = requires(T &t) { { t.~T() } noexcept; };
template<class T> constexpr bool n(T &t) { return noexcept(t.~T()); }
static_assert(A<int>);
static_assert(B<int>);
int i;
static_assert(n(i));
