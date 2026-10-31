//@ expect: compile-ok
// [expr.prim.req.compound]/1.2: `noexcept` requires a non-throwing
// expression.
template <class T> concept NoThrowCall = requires(T a) { { a.f() } noexcept; };
struct Yes { void f() noexcept; };
struct No { void f(); };
static_assert(NoThrowCall<Yes>);
static_assert(!NoThrowCall<No>);
