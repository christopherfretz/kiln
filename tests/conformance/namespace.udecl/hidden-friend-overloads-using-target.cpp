// [namespace.udecl]/10, [basic.scope.scope]/4, [namespace.memdef]/3: a hidden friend declared in
// a namespace that has a using-declarator of the same name does not redeclare the using-declarator:
// a friend whose signature differs from the target's overloads it, found by argument-dependent
// lookup, in a class or a class template, before or after the using-declaration (libstdc++
// stop_token:95 vs bits/exception_ptr.h:242, WP-273). Verified against clang 19.
//@ expect: compile-ok
namespace ep {
struct E { int p = 1; };
constexpr int swap(E&, E&) noexcept { return 1; }
template <class T> constexpr int tsw(T&) { return 10; }
}
namespace std {
struct before { friend constexpr int swap(before&, before&) noexcept { return 2; } };
using ep::swap;
using ep::tsw;
struct S { friend constexpr int swap(S&, S&) noexcept { return 3; } };
template <class T> struct TS {
  friend constexpr int swap(TS&, TS&) noexcept { return 4; }
  template <class U> friend constexpr int tsw(TS&, U) { return 5; }
};
}
constexpr int f() {
  ep::E e; std::before b; std::S s; std::TS<int> t;
  return swap(e, e) + swap(b, b) + swap(s, s) + swap(t, t) + tsw(t, 0) + std::tsw(e);
}
static_assert(f() == 1 + 2 + 3 + 4 + 5 + 10);
