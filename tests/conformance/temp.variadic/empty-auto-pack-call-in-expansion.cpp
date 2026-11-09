// [temp.variadic]/5, [temp.inst]/5: an empty `auto...` non-type pack
// expanded through a call, `vl<id(Vs)...>`. The expansion is empty, so
// `id` is never called and must not be instantiated with a placeholder
// argument type (`id<auto>`, whose deduced-return callee would then be
// "used before it is defined"). A non-empty expansion calls `id<int>`.
//@ expect: run
template <class T> T&& fwd(T& t) noexcept { return static_cast<T&&>(t); }
template <class T> constexpr T&& id(T&& t) noexcept { return fwd<T>(t); }
template <class T> constexpr auto deduced(T t) { return t; }
template <auto...> struct vl {};
template <auto... Vs> auto f() { return vl<id(Vs)...>{}; }
template <auto... Vs> auto g() { return vl<deduced(Vs)...>{}; }
using t = decltype(f<>());
using u = decltype(g<>());
using w = decltype(g<1, 'c'>());
static_assert(__is_same(u, vl<>));
static_assert(__is_same(w, vl<1, 'c'>));
int main() {}
