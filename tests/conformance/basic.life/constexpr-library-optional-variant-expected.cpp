// [basic.life]/1, /8, [class.union.general]/6, [optional.mod], [variant.mod],
// [expected.object.assign]: constant-evaluated `std::optional` (emplace after
// emplace and after reset, assignment from a value, nullopt and another
// optional, swap), `std::variant` (emplace, assignment changing the
// alternative, swap) and `std::expected` (emplace, assignment both ways),
// with the standard library's own headers, at C++23.
// libstdc++ 14 calls a member of an empty base after destroying the
// payload under it; kiln rejected that before C++26. The header-free
// reduction is constexpr-empty-base-after-member-destroyed.cpp.
//@ std: c++23
//@ expect: compile-ok
//@ strict-diags
#include <optional>
#include <variant>
#include <utility>
#if __has_include(<expected>)
#include <expected>
#endif
struct S { int v; constexpr S(int x) noexcept : v(x) {} constexpr S(const S &o) noexcept : v(o.v) {} constexpr S &operator=(const S &o) noexcept { v = o.v; return *this; } constexpr ~S() {} };
constexpr int o1() { std::optional<S> o; o.emplace(3); o.reset(); o.emplace(4); return o->v; }
constexpr int o2() { std::optional<S> o; o.emplace(3); o.emplace(4); return o->v; }
constexpr int o3() { std::optional<int> o; o.emplace(3); o.reset(); o.emplace(4); return *o; }
constexpr int o4() { std::optional<S> o; o = S(5); o = std::nullopt; o = S(6); std::optional<S> p(o); p = o; p = std::optional<S>(); p = o; return p->v; }
constexpr int o5() { std::optional<S> a(S(1)), b; a.swap(b); b.swap(a); std::swap(a, b); return a ? 0 : b->v; }
constexpr int o6() { std::optional<S> a(S(1)); a.reset(); a = S(2); a.reset(); a.emplace(3); return a->v; }
static_assert(o1() == 4); static_assert(o2() == 4); static_assert(o3() == 4);
static_assert(o4() == 6); static_assert(o5() == 1); static_assert(o6() == 3);
constexpr int v1() { std::variant<int, S> v; v.emplace<S>(3); v.emplace<int>(4); v.emplace<S>(5); return std::get<S>(v).v; }
constexpr int v2() { std::variant<int, S> v(1); v = S(2); v = 3; v = S(4); std::variant<int, S> w(v); w = v; w = 7; w = v; return std::get<1>(w).v; }
constexpr int v3() { std::variant<int, S> a(1), b(S(2)); a.swap(b); return std::get<1>(a).v + std::get<0>(b); }
static_assert(v1() == 5); static_assert(v2() == 4); static_assert(v3() == 3);
#if defined(__cpp_lib_expected)
constexpr int e1() { std::expected<S, int> e(S(1)); e = std::unexpected(2); e = S(3); e.emplace(4); return e->v; }
constexpr int e2() { std::expected<S, S> e(S(1)); e = std::unexpected(S(2)); std::expected<S, S> f(e); f = S(5); f = e; return f.error().v; }
constexpr int e3() { std::expected<int, S> e(1); e = std::unexpected(S(2)); e = 3; e.emplace(4); return *e; }
static_assert(e1() == 4); static_assert(e2() == 2); static_assert(e3() == 4);
#endif
